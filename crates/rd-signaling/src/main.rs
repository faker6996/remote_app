//! WebRTC Signaling Server
//! 
//! Lightweight server for exchanging SDP offers/answers and ICE candidates
//! between peers to establish WebRTC P2P connections.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn};

/// Signaling message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SignalMessage {
    /// Register as a peer (host)
    Register { peer_id: String },
    
    /// SDP Offer from host
    Offer { peer_id: String, sdp: String },
    
    /// SDP Answer from viewer
    Answer { peer_id: String, sdp: String },
    
    /// ICE Candidate
    IceCandidate { 
        peer_id: String, 
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u16>,
    },
    
    /// Peer registered successfully
    Registered { peer_id: String },
    
    /// Error message
    Error { message: String },
}

/// Connected peer info
#[derive(Clone)]
struct PeerInfo {
    peer_id: String,
    tx: broadcast::Sender<SignalMessage>,
}

/// Shared application state
#[derive(Clone)]
struct AppState {
    /// Map of peer_id -> PeerInfo
    peers: Arc<DashMap<String, PeerInfo>>,
    /// Map of peer_id -> SDP offer (for late joiners)
    offers: Arc<DashMap<String, String>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            peers: Arc::new(DashMap::new()),
            offers: Arc::new(DashMap::new()),
        }
    }
}

/// Generate a short peer ID (like AnyDesk)
fn generate_peer_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let id: u32 = rng.gen_range(100_000..999_999);
    format!("{}", id)
}

/// REST endpoint to get a new peer ID
async fn get_peer_id() -> Json<serde_json::Value> {
    let peer_id = generate_peer_id();
    Json(serde_json::json!({ "peer_id": peer_id }))
}

/// REST endpoint to check if peer exists
async fn check_peer(
    State(state): State<AppState>,
    Path(peer_id): Path<String>,
) -> Json<serde_json::Value> {
    let exists = state.peers.contains_key(&peer_id);
    Json(serde_json::json!({ "exists": exists }))
}

/// WebSocket handler for signaling
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let (tx, mut rx) = broadcast::channel::<SignalMessage>(16);
    let mut peer_id: Option<String> = None;
    
    loop {
        tokio::select! {
            // Receive from WebSocket
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<SignalMessage>(&text) {
                            Ok(signal) => {
                                handle_signal(&mut socket, &state, &tx, &mut peer_id, signal).await;
                            }
                            Err(e) => {
                                warn!("Invalid message: {}", e);
                                let err = SignalMessage::Error { 
                                    message: format!("Invalid message: {}", e) 
                                };
                                let _ = socket.send(Message::Text(serde_json::to_string(&err).unwrap())).await;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        break;
                    }
                    _ => {}
                }
            }
            
            // Forward messages from broadcast channel
            msg = rx.recv() => {
                if let Ok(signal) = msg {
                    if let Ok(text) = serde_json::to_string(&signal) {
                        let _ = socket.send(Message::Text(text)).await;
                    }
                }
            }
        }
    }
    
    // Cleanup on disconnect
    if let Some(id) = peer_id {
        info!("Peer {} disconnected", id);
        state.peers.remove(&id);
        state.offers.remove(&id);
    }
}

async fn handle_signal(
    socket: &mut WebSocket,
    state: &AppState,
    tx: &broadcast::Sender<SignalMessage>,
    peer_id: &mut Option<String>,
    signal: SignalMessage,
) {
    match signal {
        SignalMessage::Register { peer_id: id } => {
            info!("Peer {} registered", id);
            
            // Store peer info
            state.peers.insert(id.clone(), PeerInfo {
                peer_id: id.clone(),
                tx: tx.clone(),
            });
            
            *peer_id = Some(id.clone());
            
            // Send confirmation
            let response = SignalMessage::Registered { peer_id: id };
            let _ = socket.send(Message::Text(serde_json::to_string(&response).unwrap())).await;
        }
        
        SignalMessage::Offer { peer_id: id, sdp } => {
            info!("Offer from {}", id);
            state.offers.insert(id, sdp);
        }
        
        SignalMessage::Answer { peer_id: target_id, sdp } => {
            info!("Answer for {}", target_id);
            
            // Forward to target peer
            if let Some(peer) = state.peers.get(&target_id) {
                let msg = SignalMessage::Answer { 
                    peer_id: peer_id.clone().unwrap_or_default(), 
                    sdp 
                };
                let _ = peer.tx.send(msg);
            }
        }
        
        SignalMessage::IceCandidate { peer_id: target_id, candidate, sdp_mid, sdp_mline_index } => {
            // Forward ICE candidate to target peer
            if let Some(peer) = state.peers.get(&target_id) {
                let msg = SignalMessage::IceCandidate { 
                    peer_id: peer_id.clone().unwrap_or_default(),
                    candidate,
                    sdp_mid,
                    sdp_mline_index,
                };
                let _ = peer.tx.send(msg);
            }
        }
        
        _ => {}
    }
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("rd_signaling=info".parse().unwrap())
        )
        .init();

    let state = AppState::new();

    let app = Router::new()
        .route("/peer-id", get(get_peer_id))
        .route("/peer/:peer_id", get(check_peer))
        .route("/ws", get(ws_handler))
        .with_state(state);

    let addr = "0.0.0.0:3030";
    info!("🚀 Signaling server running on {}", addr);
    info!("   WebSocket: ws://{}/ws", addr);
    info!("   REST: http://{}/peer-id", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
