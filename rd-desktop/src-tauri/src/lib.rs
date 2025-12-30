use std::sync::Arc;
use tokio::sync::Mutex;
use rd_client::RemoteSession;
use rd_transport::quic::QuicClient;
use rd_transport::webrtc::WebRTCTransport;
use rd_transport::Transport;

/// Connection mode for the app
#[derive(Clone, PartialEq)]
enum ConnectionMode {
    None,
    Host,   // Sharing screen
    Viewer, // Viewing remote screen
}

// State to hold the active remote session
struct AppState {
    session: Option<Arc<Mutex<RemoteSession>>>,
    webrtc_transport: Option<Arc<Mutex<WebRTCTransport>>>,
    mode: ConnectionMode,
    peer_id: String,
}

impl AppState {
    fn new() -> Self {
        Self {
            session: None,
            webrtc_transport: None,
            mode: ConnectionMode::None,
            peer_id: String::new(),
        }
    }
}

/// Generate a random 6-character peer ID
fn generate_peer_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    let id = format!("{:06X}", (seed % 0xFFFFFF) as u32);
    id
}

/// Start hosting (share screen) - registers with signaling server
#[tauri::command]
async fn start_host(
    signaling_url: String,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<String, String> {
    let peer_id = generate_peer_id();
    
    // Store state
    {
        let mut app_state = state.lock().await;
        app_state.mode = ConnectionMode::Host;
        app_state.peer_id = peer_id.clone();
    }
    
    // Start WebRTC as callee (wait for incoming connection)
    let transport = WebRTCTransport::new_as_callee(&signaling_url, &peer_id)
        .await
        .map_err(|e| format!("Failed to start host: {}", e))?;
    
    // Store transport
    {
        let mut app_state = state.lock().await;
        app_state.webrtc_transport = Some(Arc::new(Mutex::new(transport)));
    }
    
    Ok(peer_id)
}

/// Connect as viewer to a remote peer
#[tauri::command]
async fn connect_peer(
    signaling_url: String,
    remote_peer_id: String,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<String, String> {
    let local_peer_id = generate_peer_id();
    
    // Store state
    {
        let mut app_state = state.lock().await;
        app_state.mode = ConnectionMode::Viewer;
        app_state.peer_id = local_peer_id.clone();
    }
    
    // Start WebRTC as caller (initiate connection)
    let transport = WebRTCTransport::new_as_caller(&signaling_url, &local_peer_id, &remote_peer_id)
        .await
        .map_err(|e| format!("Failed to connect: {}", e))?;
    
    // Store transport
    {
        let mut app_state = state.lock().await;
        app_state.webrtc_transport = Some(Arc::new(Mutex::new(transport)));
    }
    
    Ok(format!("Connected to peer {}", remote_peer_id))
}

/// Stop hosting or disconnect
#[tauri::command]
async fn stop_connection(state: tauri::State<'_, Arc<Mutex<AppState>>>) -> Result<String, String> {
    let mut app_state = state.lock().await;
    
    if let Some(transport) = &app_state.webrtc_transport {
        let mut t = transport.lock().await;
        t.close().await.map_err(|e| e.to_string())?;
    }
    
    app_state.webrtc_transport = None;
    app_state.session = None;
    app_state.mode = ConnectionMode::None;
    
    Ok("Disconnected".to_string())
}

// Legacy QUIC connection (kept for compatibility)
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
struct FrameResponse {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

#[tauri::command]
async fn get_frame(state: tauri::State<'_, Arc<Mutex<AppState>>>) -> Result<Option<FrameResponse>, String> {
    let app_state = state.lock().await;
    
    if let Some(session) = &app_state.session {
        let mut session = session.lock().await;
        if let Some(frame) = session.receive_frame().await {
            Ok(Some(FrameResponse {
                width: frame.width,
                height: frame.height,
                data: frame.data,
            }))
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

#[tauri::command]
async fn send_input(
    event_type: String,
    x: i32,
    y: i32,
    state: tauri::State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    let app_state = state.lock().await;
    
    if let Some(session) = &app_state.session {
        let mut session = session.lock().await;
        
        let event = match event_type.as_str() {
            "mouse_move" => rd_core::domain::models::InputEvent::MouseMove { x, y },
            "mouse_down" => rd_core::domain::models::InputEvent::MouseButton {
                button: rd_core::domain::models::MouseButton::Left,
                pressed: true,
            },
            "mouse_up" => rd_core::domain::models::InputEvent::MouseButton {
                button: rd_core::domain::models::MouseButton::Left,
                pressed: false,
            },
            _ => return Err(format!("Unknown event type: {}", event_type)),
        };
        
        session.send_input(event).await.map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Ok(())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = Arc::new(Mutex::new(AppState::new()));
    
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            // P2P WebRTC commands
            start_host,
            connect_peer,
            stop_connection,
            // Legacy QUIC commands
            connect_agent,
            disconnect,
            get_frame,
            send_input
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
