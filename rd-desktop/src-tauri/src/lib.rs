use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use sha2::{Digest, Sha256};
use tauri::Emitter;
use tokio::sync::{oneshot, Mutex};
use tracing::{debug, error, info};

use rd_client::RemoteSession;
use rd_codec::Encoder;
use rd_core::domain::models::{AuthToken, CodecType, EncoderConfig, FrameFormat, InputEvent, KeyCode, MouseButton, SessionId};
use rd_core::domain::ports::ProtocolMessage;
use rd_transport::quic::QuicClient;
use rd_transport::webrtc::WebRTCTransport;

/// Connection mode for the app
#[derive(Clone, PartialEq, Debug)]
enum ConnectionMode {
    None,
    Host,   // Sharing screen
    Viewer, // Viewing remote screen
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct FrameResponse {
    pub width: u32,
    pub height: u32,
    pub jpeg_base64: String,
}

// State to hold the active remote session and WebRTC workers
struct AppState {
    session: Option<Arc<Mutex<RemoteSession>>>,
    webrtc_transport: Option<WebRTCTransport>,
    mode: ConnectionMode,
    peer_id: String,
    remote_peer_id: String,
    latest_frame: Arc<Mutex<Option<FrameResponse>>>,
    host_task: Option<tokio::task::JoinHandle<()>>,
    viewer_task: Option<tokio::task::JoinHandle<()>>,
    pending_auth_tx: Option<oneshot::Sender<(bool, bool)>>, // (accepted, allow_input)
    stream_quality: Arc<AtomicU8>,
    unattended_password_hash: Arc<Mutex<Option<String>>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            session: None,
            webrtc_transport: None,
            mode: ConnectionMode::None,
            peer_id: String::new(),
            remote_peer_id: String::new(),
            latest_frame: Arc::new(Mutex::new(None)),
            host_task: None,
            viewer_task: None,
            pending_auth_tx: None,
            stream_quality: Arc::new(AtomicU8::new(70)),
            unattended_password_hash: Arc::new(Mutex::new(None)),
        }
    }
}

/// Helper function to compute SHA-256 hash of a string
fn hash_password(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Helper to get local Downloads folder across macOS, Windows, Linux
fn get_download_dir() -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            let p = std::path::PathBuf::from(user_profile).join("Downloads");
            if p.exists() {
                return p;
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let p = std::path::PathBuf::from(home).join("Downloads");
            if p.exists() {
                return p;
            }
        }
    }
    std::env::temp_dir()
}

fn rand_id() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos()
}

struct PendingIncomingFile {
    file_name: String,
    file_size: u64,
    chunks: std::collections::HashMap<u32, Vec<u8>>,
    total_chunks: u32,
}

async fn handle_file_and_clipboard_msg(
    msg: &ProtocolMessage,
    pending_files: &mut std::collections::HashMap<String, PendingIncomingFile>,
    clip_mgr: &Arc<rd_platform::ClipboardManager>,
    app_handle: &tauri::AppHandle,
) {
    match msg {
        ProtocolMessage::ClipboardSync { text } => {
            clip_mgr.set_synced(text).await;
            let _ = rd_platform::ClipboardManager::write_text(text);
            let _ = app_handle.emit("clipboard_synced", text);
        }
        ProtocolMessage::FileTransferRequest { transfer_id, file_name, file_size } => {
            info!("Incoming file transfer: {} ({} bytes)", file_name, file_size);
            pending_files.insert(transfer_id.clone(), PendingIncomingFile {
                file_name: file_name.clone(),
                file_size: *file_size,
                chunks: std::collections::HashMap::new(),
                total_chunks: 0,
            });
            let _ = app_handle.emit("file_transfer_started", serde_json::json!({
                "transfer_id": transfer_id,
                "file_name": file_name,
                "file_size": file_size,
            }));
        }
        ProtocolMessage::FileChunk { transfer_id, chunk_index, total_chunks, data } => {
            if let Some(incoming) = pending_files.get_mut(transfer_id) {
                incoming.total_chunks = *total_chunks;
                incoming.chunks.insert(*chunk_index, data.clone());
                let percent = ((incoming.chunks.len() as f32 / *total_chunks as f32) * 100.0) as u32;
                let _ = app_handle.emit("file_transfer_progress", serde_json::json!({
                    "transfer_id": transfer_id,
                    "file_name": incoming.file_name,
                    "percent": percent,
                    "is_sender": false,
                }));
            }
        }
        ProtocolMessage::FileTransferComplete { transfer_id } => {
            if let Some(incoming) = pending_files.remove(transfer_id) {
                let mut full_data = Vec::with_capacity(incoming.file_size as usize);
                for i in 0..incoming.total_chunks {
                    if let Some(c) = incoming.chunks.get(&i) {
                        full_data.extend_from_slice(c);
                    }
                }
                let target_dir = get_download_dir();
                let save_path = target_dir.join(&incoming.file_name);
                match tokio::fs::write(&save_path, &full_data).await {
                    Ok(_) => {
                        info!("File saved successfully to {:?}", save_path);
                        let _ = app_handle.emit("file_transfer_complete", serde_json::json!({
                            "transfer_id": transfer_id,
                            "file_name": incoming.file_name,
                            "saved_path": save_path.to_string_lossy(),
                            "file_size": incoming.file_size,
                        }));
                    }
                    Err(e) => {
                        error!("Failed to write file {:?}: {}", save_path, e);
                    }
                }
            }
        }
        _ => {}
    }
}

/// Generate a random 6-character peer ID (e.g. "8F2A1C")
fn generate_peer_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    format!("{:06X}", (seed % 0xFFFFFF) as u32)
}

/// Start hosting (share screen) - registers with signaling server and waits for connections
#[tauri::command]
async fn start_host(
    signaling_url: String,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let peer_id = generate_peer_id();
    
    // Stop any existing session
    {
        let mut app_state = state.lock().await;
        if let Some(h) = app_state.host_task.take() {
            h.abort();
        }
        if let Some(h) = app_state.viewer_task.take() {
            h.abort();
        }
        if let Some(transport) = app_state.webrtc_transport.take() {
            let _ = transport.close().await;
        }
        *app_state.latest_frame.lock().await = None;
        app_state.mode = ConnectionMode::Host;
        app_state.peer_id = peer_id.clone();
        app_state.remote_peer_id.clear();
        app_state.pending_auth_tx = None;
    }
    
    let host_peer_id = peer_id.clone();
    let state_clone = state.inner().clone();
    let app_handle_clone = app_handle.clone();
    let sig_url = signaling_url.clone();
    
    // Spawn background host worker to wait for incoming connection and run capture loop
    let handle = tokio::spawn(async move {
        info!("Host background worker starting for peer {}", host_peer_id);
        
        let transport = match WebRTCTransport::new_as_callee(&sig_url, &host_peer_id).await {
            Ok(t) => t,
            Err(e) => {
                error!("Failed to initialize WebRTC host: {}", e);
                let _ = app_handle_clone.emit("host_status", format!("Host error: {}", e));
                return;
            }
        };
        
        let remote_peer = transport.remote_peer_id().to_string();
        info!("Viewer connected via WebRTC: {}", remote_peer);
        
        // Wait for SessionRequest handshake
        let auth_rx = {
            let (tx, rx) = oneshot::channel();
            let mut s = state_clone.lock().await;
            s.pending_auth_tx = Some(tx);
            s.webrtc_transport = Some(transport.clone());
            s.remote_peer_id = remote_peer.clone();
            rx
        };
        
        let pwd_hash_ref = {
            let s = state_clone.lock().await;
            s.unattended_password_hash.clone()
        };
        
        let mut auto_accepted = false;
        let mut received_auth_token: Option<String> = None;
        
        // Wait for initial handshake message (SessionRequest or Auth)
        let handshake_req = transport.recv_msg().await;
        match handshake_req {
            Ok(ProtocolMessage::SessionRequest { .. }) => {
                info!("Received SessionRequest from viewer {}", remote_peer);
                // Check if an Auth message follows within 600ms
                if let Ok(Ok(ProtocolMessage::Auth { token })) = tokio::time::timeout(
                    tokio::time::Duration::from_millis(600),
                    transport.recv_msg(),
                ).await {
                    received_auth_token = Some(token.token);
                }
            }
            Ok(ProtocolMessage::Auth { token }) => {
                received_auth_token = Some(token.token);
            }
            Ok(other) => {
                debug!("Received initial message: {:?}", other);
            }
            Err(e) => {
                error!("Handshake failed: {}", e);
                return;
            }
        }
        
        // Check if unattended access password matches
        if let Some(stored_hash) = pwd_hash_ref.lock().await.as_ref() {
            if let Some(client_token) = received_auth_token {
                if client_token == *stored_hash {
                    info!("Unattended access password verified for peer {}", remote_peer);
                    auto_accepted = true;
                }
            }
        }
        
        let decision = if auto_accepted {
            (true, true) // Auto-accept with full input permissions
        } else {
            // Prompt Host user via UI event
            let _ = app_handle_clone.emit("connection_request", serde_json::json!({
                "remote_peer_id": remote_peer
            }));
            
            // Wait for Host decision (with 60-second timeout)
            match tokio::time::timeout(tokio::time::Duration::from_secs(60), auth_rx).await {
                Ok(Ok((accepted, allow_input))) => (accepted, allow_input),
                _ => (false, false),
            }
        };
        
        if !decision.0 {
            info!("Connection rejected by host for peer {}", remote_peer);
            let _ = transport.send_msg(ProtocolMessage::SessionEnd {
                session_id: SessionId::new(),
                reason: "Connection declined by host".into(),
            }).await;
            let _ = transport.close().await;
            let _ = app_handle_clone.emit("peer_disconnected", ());
            return;
        }
        
        let allow_input = decision.1;
        info!("Connection accepted by host (allow_input={})", allow_input);
        
        // Send SessionCreated to confirm acceptance
        let _ = transport.send_msg(ProtocolMessage::SessionCreated {
            session_id: SessionId::new(),
            endpoint: "p2p".into(),
        }).await;
        
        let _ = app_handle_clone.emit("peer_connected", serde_json::json!({
            "remote_peer_id": remote_peer
        }));
        
        // 1. Screen capture & JPEG encode task
        let transport_send = transport.clone();
        let quality_ref = {
            let s = state_clone.lock().await;
            s.stream_quality.clone()
        };
        
        let mut capture_task = tokio::spawn(async move {
            let capture = match rd_platform::create_screen_capture() {
                Ok(c) => c,
                Err(e) => {
                    error!("Failed to create screen capture: {}", e);
                    return;
                }
            };
            
            let mut encoder = rd_codec::JpegEncoder::with_quality(quality_ref.load(Ordering::Relaxed));
            let mut ticker = tokio::time::interval(tokio::time::Duration::from_millis(33)); // ~30 FPS
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let mut seq = 0u64;
            let mut current_q = quality_ref.load(Ordering::Relaxed);
            
            info!("Screen capture loop started (target: 30 FPS)");
            loop {
                ticker.tick().await;
                
                // Update encoder quality if user changed setting
                let new_q = quality_ref.load(Ordering::Relaxed);
                if new_q != current_q {
                    current_q = new_q;
                    let _ = encoder.set_config(EncoderConfig {
                        codec: CodecType::Jpeg,
                        quality: current_q,
                        target_fps: 30,
                        bitrate: None,
                    });
                }
                
                let frame = match capture.lock().await.capture().await {
                    Ok(f) => f,
                    Err(e) => {
                        debug!("Screen capture frame dropped: {}", e);
                        continue;
                    }
                };
                
                let jpeg_bytes = match encoder.encode(&frame).await {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        error!("Frame encode error: {}", e);
                        continue;
                    }
                };
                
                const MAX_CHUNK_SIZE: usize = 50_000;
                if jpeg_bytes.len() <= MAX_CHUNK_SIZE {
                    let msg = ProtocolMessage::ScreenFrame {
                        sequence: seq,
                        timestamp: frame.timestamp,
                        data: jpeg_bytes,
                        width: frame.width,
                        height: frame.height,
                        format: FrameFormat::Jpeg,
                    };
                    
                    if let Err(e) = transport_send.send_msg(msg).await {
                        error!("Failed to send frame over WebRTC: {}", e);
                        break;
                    }
                } else {
                    let total_chunks = ((jpeg_bytes.len() + MAX_CHUNK_SIZE - 1) / MAX_CHUNK_SIZE) as u32;
                    let mut send_failed = false;
                    for (chunk_idx, chunk_data) in jpeg_bytes.chunks(MAX_CHUNK_SIZE).enumerate() {
                        let msg = ProtocolMessage::FrameChunk {
                            sequence: seq,
                            timestamp: frame.timestamp,
                            chunk_index: chunk_idx as u32,
                            total_chunks,
                            data: chunk_data.to_vec(),
                            width: frame.width,
                            height: frame.height,
                            format: FrameFormat::Jpeg,
                        };
                        if let Err(e) = transport_send.send_msg(msg).await {
                            error!("Failed to send frame chunk {}/{} over WebRTC: {}", chunk_idx + 1, total_chunks, e);
                            send_failed = true;
                            break;
                        }
                    }
                    if send_failed {
                        break;
                    }
                }
                seq += 1;
            }
        });
        
        // 2. Audio capture & streaming task
        let transport_audio = transport.clone();
        let (_audio_service, audio_rx) = rd_platform::AudioCaptureService::start_capture();
        let mut audio_task = tokio::spawn(async move {
            if let Some(mut rx) = audio_rx {
                let mut a_seq = 0u64;
                let mut buffer = Vec::with_capacity(9600);
                while let Some(pcm) = rx.recv().await {
                    buffer.extend_from_slice(&pcm);
                    if buffer.len() >= 4800 {
                        let to_send = std::mem::take(&mut buffer);
                        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
                        let msg = ProtocolMessage::AudioFrame {
                            sequence: a_seq,
                            timestamp: now,
                            sample_rate: 48000,
                            channels: 2,
                            data: to_send,
                        };
                        if let Err(_) = transport_audio.send_msg(msg).await {
                            break;
                        }
                        a_seq += 1;
                    }
                }
            }
        });

        // 3. Local clipboard monitor & sync task
        let transport_clip = transport.clone();
        let clip_mgr_host = Arc::new(rd_platform::ClipboardManager::new());
        let clip_mgr_clone = clip_mgr_host.clone();
        let mut clipboard_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(400));
            loop {
                interval.tick().await;
                if let Some(text) = clip_mgr_clone.get_new_text_if_changed().await {
                    let _ = transport_clip.send_msg(ProtocolMessage::ClipboardSync { text }).await;
                }
            }
        });

        // 4. Remote message receiver task (input injection, clipboard, file transfer)
        let transport_recv = transport.clone();
        let app_handle_for_recv = app_handle_clone.clone();
        let clip_mgr_for_recv = clip_mgr_host.clone();
        let mut receiver_task = tokio::spawn(async move {
            let injector = if allow_input {
                rd_platform::create_input_injector().ok()
            } else {
                None
            };
            let mut pending_files = std::collections::HashMap::new();
            
            info!("Host remote message receiver started");
            loop {
                let msg = match transport_recv.recv_msg().await {
                    Ok(m) => m,
                    Err(e) => {
                        info!("Host input receiver stopped: {}", e);
                        break;
                    }
                };
                
                match &msg {
                    ProtocolMessage::SessionEnd { reason, .. } => {
                        info!("Remote viewer ended session: {}", reason);
                        break;
                    }
                    ProtocolMessage::InputEvent { event, .. } => {
                        if let Some(ref inj) = injector {
                            if let Err(e) = inj.lock().await.inject(event.clone()).await {
                                error!("Failed to inject input event: {}", e);
                            }
                        }
                    }
                    _ => {
                        handle_file_and_clipboard_msg(
                            &msg,
                            &mut pending_files,
                            &clip_mgr_for_recv,
                            &app_handle_for_recv,
                        ).await;
                    }
                }
            }
        });
        
        // Wait until connection drops or stop_connection is called
        tokio::select! {
            _ = &mut capture_task => {},
            _ = &mut audio_task => {},
            _ = &mut clipboard_task => {},
            _ = &mut receiver_task => {},
        }
        
        capture_task.abort();
        audio_task.abort();
        clipboard_task.abort();
        receiver_task.abort();
        
        info!("Host session ended");
        let _ = app_handle_clone.emit("peer_disconnected", ());
    });
    
    {
        let mut app_state = state.lock().await;
        app_state.host_task = Some(handle);
    }
    
    Ok(peer_id)
}

/// Accept or decline an incoming connection request on Host
#[tauri::command]
async fn respond_connection_request(
    accepted: bool,
    allow_input: bool,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    let mut app_state = state.lock().await;
    if let Some(tx) = app_state.pending_auth_tx.take() {
        let _ = tx.send((accepted, allow_input));
    }
    Ok(())
}

/// Configure unattended access password on Host (empty password disables it)
#[tauri::command]
async fn set_unattended_password(
    password: String,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<bool, String> {
    let app_state = state.lock().await;
    let mut hash_lock = app_state.unattended_password_hash.lock().await;
    if password.trim().is_empty() {
        *hash_lock = None;
        info!("Unattended access disabled");
        Ok(false)
    } else {
        let hash = hash_password(password.trim());
        *hash_lock = Some(hash);
        info!("Unattended access enabled with password hash");
        Ok(true)
    }
}

/// Adjust streaming quality dynamically (30 - 95)
#[tauri::command]
async fn set_stream_quality(
    quality: u8,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    let app_state = state.lock().await;
    app_state.stream_quality.store(quality.clamp(30, 95), Ordering::Relaxed);
    Ok(())
}

/// Connect as viewer to a remote peer - initiates session request with optional password
#[tauri::command]
async fn connect_peer(
    signaling_url: String,
    remote_peer_id: String,
    password: Option<String>,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let local_peer_id = generate_peer_id();
    
    // Stop any existing session
    {
        let mut app_state = state.lock().await;
        if let Some(h) = app_state.host_task.take() {
            h.abort();
        }
        if let Some(h) = app_state.viewer_task.take() {
            h.abort();
        }
        if let Some(transport) = app_state.webrtc_transport.take() {
            let _ = transport.close().await;
        }
        *app_state.latest_frame.lock().await = None;
        app_state.mode = ConnectionMode::Viewer;
        app_state.peer_id = local_peer_id.clone();
        app_state.remote_peer_id = remote_peer_id.clone();
    }
    
    // Start WebRTC connection as caller
    let transport = WebRTCTransport::new_as_caller(&signaling_url, &local_peer_id, &remote_peer_id)
        .await
        .map_err(|e| format!("Failed to connect to peer {}: {}", remote_peer_id, e))?;
    
    // Send SessionRequest to host
    transport.send_msg(ProtocolMessage::SessionRequest {
        target_device: remote_peer_id.clone(),
    }).await.map_err(|e| format!("Failed to send SessionRequest: {}", e))?;
    
    // If password provided, send Auth token
    if let Some(pwd) = password {
        if !pwd.trim().is_empty() {
            let hash = hash_password(pwd.trim());
            let _ = transport.send_msg(ProtocolMessage::Auth {
                token: AuthToken::new(hash, local_peer_id.clone()),
            }).await;
        }
    }
    
    // Wait for host acceptance
    let response = transport.recv_msg().await
        .map_err(|e| format!("Failed to receive session response: {}", e))?;
        
    match response {
        ProtocolMessage::SessionCreated { .. } => {
            info!("Session accepted by remote host {}", remote_peer_id);
        }
        ProtocolMessage::SessionEnd { reason, .. } => {
            return Err(format!("Connection rejected: {}", reason));
        }
        other => {
            debug!("Received handshake message: {:?}", other);
        }
    }
    
    // Spawn background receiver task to decode incoming frames and handle messages
    let transport_recv = transport.clone();
    let latest_frame = {
        let app_state = state.lock().await;
        app_state.latest_frame.clone()
    };
    let app_handle_clone = app_handle.clone();
    
    // Viewer local clipboard monitor & sync task
    let transport_clip = transport.clone();
    let clip_mgr_viewer = Arc::new(rd_platform::ClipboardManager::new());
    let clip_mgr_clone = clip_mgr_viewer.clone();
    let mut viewer_clip_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(400));
        loop {
            interval.tick().await;
            if let Some(text) = clip_mgr_clone.get_new_text_if_changed().await {
                let _ = transport_clip.send_msg(ProtocolMessage::ClipboardSync { text }).await;
            }
        }
    });
    
    let clip_mgr_for_viewer = clip_mgr_viewer.clone();
    let mut viewer_frame_task = tokio::spawn(async move {
        let mut pending_files = std::collections::HashMap::new();
        info!("Viewer frame receiver task started");
        
        struct PendingChunkedFrame {
            sequence: u64,
            total_chunks: u32,
            chunks: Vec<Option<Vec<u8>>>,
            received_count: u32,
            width: u32,
            height: u32,
            format: FrameFormat,
        }
        
        let mut pending_frame: Option<PendingChunkedFrame> = None;
        
        loop {
            let msg = match transport_recv.recv_msg().await {
                Ok(m) => m,
                Err(e) => {
                    info!("Viewer receiver stream ended: {}", e);
                    break;
                }
            };
            
            match msg {
                ProtocolMessage::ScreenFrame { data, width, height, .. } => {
                    // Reset any pending chunked frame on complete frame arrival
                    pending_frame = None;
                    use base64::prelude::*;
                    let jpeg_b64 = BASE64_STANDARD.encode(&data);
                    
                    let mut slot = latest_frame.lock().await;
                    *slot = Some(FrameResponse {
                        width,
                        height,
                        jpeg_base64: jpeg_b64,
                    });
                }
                ProtocolMessage::FrameChunk {
                    sequence,
                    chunk_index,
                    total_chunks,
                    data,
                    width,
                    height,
                    format,
                    ..
                } => {
                    // If chunk belongs to an older frame than currently assembling, ignore
                    if let Some(ref pf) = pending_frame {
                        if sequence < pf.sequence {
                            continue;
                        }
                    }
                    
                    // Reset if this chunk belongs to a newer frame
                    let should_reset = match &pending_frame {
                        Some(pf) => pf.sequence != sequence,
                        None => true,
                    };
                    
                    if should_reset {
                        pending_frame = Some(PendingChunkedFrame {
                            sequence,
                            total_chunks,
                            chunks: vec![None; total_chunks as usize],
                            received_count: 0,
                            width,
                            height,
                            format,
                        });
                    }
                    
                    if let Some(ref mut pf) = pending_frame {
                        let idx = chunk_index as usize;
                        if idx < pf.chunks.len() && pf.chunks[idx].is_none() {
                            pf.chunks[idx] = Some(data);
                            pf.received_count += 1;
                        }
                        
                        // Check if all chunks for this frame have arrived
                        if pf.received_count == pf.total_chunks {
                            let total_bytes: usize = pf.chunks.iter().map(|c| c.as_ref().map(|v| v.len()).unwrap_or(0)).sum();
                            let mut full_data = Vec::with_capacity(total_bytes);
                            for chunk in pf.chunks.iter() {
                                if let Some(bytes) = chunk {
                                    full_data.extend_from_slice(bytes);
                                }
                            }
                            let w = pf.width;
                            let h = pf.height;
                            pending_frame = None; // Reset for next frame
                            
                            use base64::prelude::*;
                            let jpeg_b64 = BASE64_STANDARD.encode(&full_data);
                            
                            let mut slot = latest_frame.lock().await;
                            *slot = Some(FrameResponse {
                                width: w,
                                height: h,
                                jpeg_base64: jpeg_b64,
                            });
                        }
                    }
                }
                ProtocolMessage::AudioFrame { sample_rate, channels, data, .. } => {
                    let _ = app_handle_clone.emit("remote_audio_frame", serde_json::json!({
                        "sample_rate": sample_rate,
                        "channels": channels,
                        "data": data,
                    }));
                }
                ProtocolMessage::SessionEnd { reason, .. } => {
                    info!("Remote host ended session: {}", reason);
                    break;
                }
                other => {
                    handle_file_and_clipboard_msg(
                        &other,
                        &mut pending_files,
                        &clip_mgr_for_viewer,
                        &app_handle_clone,
                    ).await;
                }
            }
        }
        
        info!("Viewer session ended");
        let _ = app_handle_clone.emit("peer_disconnected", ());
    });
    
    let handle = tokio::spawn(async move {
        tokio::select! {
            _ = &mut viewer_frame_task => {},
            _ = &mut viewer_clip_task => {},
        }
        viewer_frame_task.abort();
        viewer_clip_task.abort();
    });
    
    {
        let mut app_state = state.lock().await;
        app_state.webrtc_transport = Some(transport);
        app_state.viewer_task = Some(handle);
    }
    
    Ok(format!("Connected to peer {}", remote_peer_id))
}

/// Send a file to connected remote peer (works from either Host or Viewer)
#[tauri::command]
async fn send_file(
    file_name: String,
    file_data: Vec<u8>,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let transport = {
        let app_state = state.lock().await;
        app_state.webrtc_transport.clone()
    }.ok_or("No active connection")?;
    
    let transfer_id = format!("{:08x}", rand_id());
    let file_size = file_data.len() as u64;
    
    // Notify peer of incoming file transfer
    transport.send_msg(ProtocolMessage::FileTransferRequest {
        transfer_id: transfer_id.clone(),
        file_name: file_name.clone(),
        file_size,
    }).await.map_err(|e| format!("Failed to send file transfer request: {}", e))?;
    
    const CHUNK_SIZE: usize = 32_000;
    let total_chunks = ((file_data.len() + CHUNK_SIZE - 1) / CHUNK_SIZE).max(1) as u32;
    
    for (i, chunk) in file_data.chunks(CHUNK_SIZE).enumerate() {
        let chunk_index = i as u32;
        transport.send_msg(ProtocolMessage::FileChunk {
            transfer_id: transfer_id.clone(),
            chunk_index,
            total_chunks,
            data: chunk.to_vec(),
        }).await.map_err(|e| format!("Failed to send file chunk: {}", e))?;
        
        let percent = (((i + 1) as f32 / total_chunks as f32) * 100.0) as u32;
        let _ = app_handle.emit("file_transfer_progress", serde_json::json!({
            "transfer_id": transfer_id,
            "file_name": file_name,
            "percent": percent,
            "is_sender": true,
        }));
    }
    
    transport.send_msg(ProtocolMessage::FileTransferComplete {
        transfer_id: transfer_id.clone(),
    }).await.map_err(|e| format!("Failed to complete file transfer: {}", e))?;
    
    info!("File {} ({} bytes) sent successfully", file_name, file_size);
    Ok(format!("Sent file {}", file_name))
}

/// Stop hosting or disconnect from remote peer
#[tauri::command]
async fn stop_connection(
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    info!("stop_connection called");
    
    // 1. Extract resources and release app_state lock IMMEDIATELY (no async inside lock!)
    let (host_task, viewer_task, transport, latest_frame) = {
        let mut app_state = state.lock().await;
        let ht = app_state.host_task.take();
        let vt = app_state.viewer_task.take();
        let tr = app_state.webrtc_transport.take();
        let lf = app_state.latest_frame.clone();
        app_state.session = None;
        app_state.mode = ConnectionMode::None;
        app_state.remote_peer_id.clear();
        app_state.pending_auth_tx = None;
        (ht, vt, tr, lf)
    };
    
    // 2. Clear latest frame
    *latest_frame.lock().await = None;
    
    // 3. Abort background worker tasks
    if let Some(h) = host_task {
        h.abort();
    }
    if let Some(h) = viewer_task {
        h.abort();
    }
    
    // 4. Send SessionEnd message and close transport asynchronously in background
    // with a strict 800ms timeout so it NEVER blocks the caller!
    if let Some(tr) = transport {
        tokio::spawn(async move {
            let _ = tr.send_msg(ProtocolMessage::SessionEnd {
                session_id: SessionId::new(),
                reason: "User requested disconnect".to_string(),
            }).await;
            let _ = tokio::time::timeout(tokio::time::Duration::from_millis(800), tr.close()).await;
        });
    }
    
    // 5. Emit peer_disconnected event locally so UI updates instantly
    let _ = app_handle.emit("peer_disconnected", ());
    
    Ok("Disconnected".to_string())
}

/// Get the latest screen frame for UI canvas rendering
#[tauri::command]
async fn get_frame(state: tauri::State<'_, Arc<Mutex<AppState>>>) -> Result<Option<FrameResponse>, String> {
    let app_state = state.lock().await;
    
    // 1. WebRTC P2P session
    {
        let frame = app_state.latest_frame.lock().await.clone();
        if frame.is_some() {
            return Ok(frame);
        }
    }
    
    // 2. Legacy QUIC session fallback
    if let Some(session) = &app_state.session {
        let mut session = session.lock().await;
        if let Some(frame) = session.receive_frame().await {
            use base64::prelude::*;
            let jpeg_b64 = BASE64_STANDARD.encode(&frame.data);
            return Ok(Some(FrameResponse {
                width: frame.width,
                height: frame.height,
                jpeg_base64: jpeg_b64,
            }));
        }
    }
    
    Ok(None)
}

/// Forward mouse input event to remote host
#[tauri::command]
async fn send_input(
    event_type: String,
    x: i32,
    y: i32,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    let app_state = state.lock().await;
    
    let event = match event_type.as_str() {
        "mouse_move" => InputEvent::MouseMove { x, y },
        "mouse_down" => InputEvent::MouseButton {
            button: MouseButton::Left,
            pressed: true,
        },
        "mouse_up" => InputEvent::MouseButton {
            button: MouseButton::Left,
            pressed: false,
        },
        "right_mouse_down" => InputEvent::MouseButton {
            button: MouseButton::Right,
            pressed: true,
        },
        "right_mouse_up" => InputEvent::MouseButton {
            button: MouseButton::Right,
            pressed: false,
        },
        "mouse_scroll" => InputEvent::MouseScroll {
            delta_x: x,
            delta_y: y,
        },
        _ => return Err(format!("Unknown event type: {}", event_type)),
    };
    
    // Send over WebRTC DataChannel if active
    if let Some(transport) = &app_state.webrtc_transport {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // When mouse button is clicked, first position cursor at exact (x, y) coordinates
        if event_type == "mouse_down" || event_type == "right_mouse_down" {
            let _ = transport.send_msg(ProtocolMessage::InputEvent {
                timestamp: now,
                event: InputEvent::MouseMove { x, y },
            }).await;
        }

        let msg = ProtocolMessage::InputEvent {
            timestamp: now,
            event: event.clone(),
        };
        if let Err(e) = transport.send_msg(msg).await {
            error!("Failed to send mouse input over WebRTC: {}", e);
        }
    }
    
    // Legacy QUIC session fallback
    if let Some(session) = &app_state.session {
        let mut session = session.lock().await;
        session.send_input(event).await.map_err(|e| e.to_string())?;
    }
    
    Ok(())
}

/// Forward keyboard key press/release event to remote host
#[tauri::command]
async fn send_key(
    key_code: u32,
    pressed: bool,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    let app_state = state.lock().await;
    
    if let Some(transport) = &app_state.webrtc_transport {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let msg = ProtocolMessage::InputEvent {
            timestamp: now,
            event: InputEvent::KeyPress {
                key: KeyCode(key_code),
                pressed,
            },
        };
        if let Err(e) = transport.send_msg(msg).await {
            error!("Failed to send key input over WebRTC: {}", e);
        }
    }
    
    Ok(())
}

// Legacy QUIC connection (kept for CLI & compatibility)
#[tauri::command]
async fn connect_agent(
    server_addr: String,
    agent_id: String,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<String, String> {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    
    let client = QuicClient::new().map_err(|e| e.to_string())?;
    let connection = client
        .connect(server_addr.parse().map_err(|e: std::net::AddrParseError| e.to_string())?)
        .await
        .map_err(|e| e.to_string())?;
    
    let transport = rd_transport::QuicTransport::new(connection)
        .await
        .map_err(|e| e.to_string())?;
    
    let mut session = RemoteSession::new(Arc::new(Mutex::new(transport)))
        .await
        .map_err(|e| e.to_string())?;
    
    let session_id = session
        .connect(agent_id.clone())
        .await
        .map_err(|e| e.to_string())?;
    
    let mut app_state = state.lock().await;
    app_state.session = Some(Arc::new(Mutex::new(session)));
    
    Ok(format!("Connected to agent {} with session {}", agent_id, session_id))
}

#[tauri::command]
async fn disconnect(state: tauri::State<'_, Arc<Mutex<AppState>>>) -> Result<String, String> {
    let mut app_state = state.lock().await;
    
    if let Some(session) = &app_state.session {
        session.lock().await.disconnect().await.map_err(|e| e.to_string())?;
        app_state.session = None;
        Ok("Disconnected".to_string())
    } else {
        Err("No active session".to_string())
    }
}

#[derive(serde::Serialize)]
pub struct SystemPermissions {
    pub screen_recording: bool,
    pub accessibility: bool,
}

#[tauri::command]
fn check_permissions() -> SystemPermissions {
    let (screen, a11y) = rd_platform::check_system_permissions();
    SystemPermissions {
        screen_recording: screen,
        accessibility: a11y,
    }
}

#[tauri::command]
fn open_accessibility_settings() {
    rd_platform::open_accessibility_settings();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let app_state = Arc::new(Mutex::new(AppState::new()));
    
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            // P2P WebRTC commands
            start_host,
            connect_peer,
            stop_connection,
            respond_connection_request,
            set_stream_quality,
            set_unattended_password,
            send_key,
            send_file,
            check_permissions,
            open_accessibility_settings,
            // Legacy QUIC commands
            connect_agent,
            disconnect,
            get_frame,
            send_input
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
