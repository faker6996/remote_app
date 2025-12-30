//! Signaling client for WebRTC connection establishment
//! 
//! Connects to the signaling server via WebSocket to exchange
//! SDP offers/answers and ICE candidates.

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};
use url::Url;

/// Signaling message types (must match rd-signaling)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SignalMessage {
    Register { peer_id: String },
    Offer { peer_id: String, sdp: String },
    Answer { peer_id: String, sdp: String },
    IceCandidate { 
        peer_id: String, 
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u16>,
    },
    Registered { peer_id: String },
    Error { message: String },
}

/// Signaling client for WebRTC setup
pub struct SignalingClient {
    tx: mpsc::Sender<SignalMessage>,
    rx: mpsc::Receiver<SignalMessage>,
    peer_id: String,
}

impl SignalingClient {
    /// Connect to signaling server
    pub async fn connect(signaling_url: &str, peer_id: &str) -> Result<Self, anyhow::Error> {
        let url = Url::parse(&format!("{}/ws", signaling_url))?;
        info!("Connecting to signaling server: {}", url);
        
        let (ws_stream, _) = connect_async(url).await?;
        let (mut ws_tx, mut ws_rx) = ws_stream.split();
        
        // Channels for message passing
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<SignalMessage>(16);
        let (incoming_tx, incoming_rx) = mpsc::channel::<SignalMessage>(16);
        
        let peer_id_clone = peer_id.to_string();
        
        // Spawn task to handle WebSocket I/O
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Forward outgoing messages to WebSocket
                    Some(msg) = outgoing_rx.recv() => {
                        if let Ok(json) = serde_json::to_string(&msg) {
                            if ws_tx.send(Message::Text(json)).await.is_err() {
                                break;
                            }
                        }
                    }
                    
                    // Receive from WebSocket and forward
                    msg = ws_rx.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                if let Ok(signal) = serde_json::from_str::<SignalMessage>(&text) {
                                    debug!("Received signal: {:?}", signal);
                                    if incoming_tx.send(signal).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            Some(Ok(Message::Close(_))) | None => break,
                            Some(Err(e)) => {
                                error!("WebSocket error: {}", e);
                                break;
                            }
                            _ => {}
                        }
                    }
                }
            }
            warn!("Signaling connection closed");
        });
        
        // Register with signaling server
        let register = SignalMessage::Register { peer_id: peer_id.to_string() };
        outgoing_tx.send(register).await?;
        
        Ok(Self {
            tx: outgoing_tx,
            rx: incoming_rx,
            peer_id: peer_id_clone,
        })
    }
    
    /// Send SDP offer
    pub async fn send_offer(&self, target_peer: &str, sdp: &str) -> Result<(), anyhow::Error> {
        let msg = SignalMessage::Offer {
            peer_id: target_peer.to_string(),
            sdp: sdp.to_string(),
        };
        self.tx.send(msg).await?;
        Ok(())
    }
    
    /// Send SDP answer
    pub async fn send_answer(&self, target_peer: &str, sdp: &str) -> Result<(), anyhow::Error> {
        let msg = SignalMessage::Answer {
            peer_id: target_peer.to_string(),
            sdp: sdp.to_string(),
        };
        self.tx.send(msg).await?;
        Ok(())
    }
    
    /// Send ICE candidate
    pub async fn send_ice_candidate(
        &self,
        target_peer: &str,
        candidate: &str,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u16>,
    ) -> Result<(), anyhow::Error> {
        let msg = SignalMessage::IceCandidate {
            peer_id: target_peer.to_string(),
            candidate: candidate.to_string(),
            sdp_mid,
            sdp_mline_index,
        };
        self.tx.send(msg).await?;
        Ok(())
    }
    
    /// Receive next signal message
    pub async fn recv(&mut self) -> Option<SignalMessage> {
        self.rx.recv().await
    }
    
    /// Get our peer ID
    pub fn peer_id(&self) -> &str {
        &self.peer_id
    }
}
