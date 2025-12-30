//! WebRTC Transport Implementation
//! 
//! Implements the Transport trait using WebRTC DataChannels for P2P
//! communication between peers.

use std::sync::Arc;
use async_trait::async_trait;
use rd_core::domain::{
    error::TransportError,
    ports::{ProtocolMessage, Transport},
};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info};
use webrtc::{
    api::APIBuilder,
    data_channel::{data_channel_message::DataChannelMessage, RTCDataChannel},
    ice_transport::ice_server::RTCIceServer,
    peer_connection::{
        configuration::RTCConfiguration,
        peer_connection_state::RTCPeerConnectionState,
        sdp::session_description::RTCSessionDescription,
        RTCPeerConnection,
    },
};

use super::signaling::SignalingClient;

/// WebRTC-based P2P transport
pub struct WebRTCTransport {
    peer_connection: Arc<RTCPeerConnection>,
    data_channel: Arc<RTCDataChannel>,
    rx: mpsc::Receiver<Vec<u8>>,
    signaling: Arc<Mutex<SignalingClient>>,
    remote_peer_id: String,
}

impl WebRTCTransport {
    /// Create a new WebRTC transport as the initiator (caller)
    pub async fn new_as_caller(
        signaling_url: &str,
        local_peer_id: &str,
        remote_peer_id: &str,
    ) -> Result<Self, anyhow::Error> {
        info!("Creating WebRTC transport as caller to peer {}", remote_peer_id);
        
        // Connect to signaling server
        let signaling = SignalingClient::connect(signaling_url, local_peer_id).await?;
        let signaling = Arc::new(Mutex::new(signaling));
        
        // Create WebRTC peer connection with STUN servers
        let config = RTCConfiguration {
            ice_servers: vec![
                RTCIceServer {
                    urls: vec![
                        "stun:stun.l.google.com:19302".to_string(),
                        "stun:stun1.l.google.com:19302".to_string(),
                    ],
                    ..Default::default()
                }
            ],
            ..Default::default()
        };
        
        let api = APIBuilder::new().build();
        let peer_connection = Arc::new(api.new_peer_connection(config).await?);
        
        // Create data channel for messaging
        let data_channel = peer_connection.create_data_channel("remote-desktop", None).await?;
        
        // Channel for received messages
        let (tx, rx) = mpsc::channel::<Vec<u8>>(64);
        
        // Handle incoming messages on data channel
        let tx_clone = tx.clone();
        data_channel.on_message(Box::new(move |msg: DataChannelMessage| {
            let tx = tx_clone.clone();
            Box::pin(async move {
                let _ = tx.send(msg.data.to_vec()).await;
            })
        }));
        
        // Handle ICE candidates
        let signaling_ice = signaling.clone();
        let remote_peer = remote_peer_id.to_string();
        peer_connection.on_ice_candidate(Box::new(move |candidate| {
            let signaling = signaling_ice.clone();
            let peer_id = remote_peer.clone();
            Box::pin(async move {
                if let Some(c) = candidate {
                    if let Ok(json) = c.to_json() {
                        let mut sig = signaling.lock().await;
                        let _ = sig.send_ice_candidate(
                            &peer_id,
                            &json.candidate,
                            json.sdp_mid,
                            json.sdp_mline_index,
                        ).await;
                    }
                }
            })
        }));
        
        // Create and send offer
        let offer = peer_connection.create_offer(None).await?;
        peer_connection.set_local_description(offer.clone()).await?;
        
        {
            let mut sig = signaling.lock().await;
            sig.send_offer(remote_peer_id, &offer.sdp).await?;
        }
        
        info!("Sent offer to {}, waiting for answer...", remote_peer_id);
        
        // Wait for answer from signaling
        loop {
            let mut sig = signaling.lock().await;
            if let Some(msg) = sig.recv().await {
                match msg {
                    super::signaling::SignalMessage::Answer { sdp, .. } => {
                        info!("Received answer from peer");
                        let answer = RTCSessionDescription::answer(sdp)?;
                        drop(sig);
                        peer_connection.set_remote_description(answer).await?;
                        break;
                    }
                    super::signaling::SignalMessage::IceCandidate { candidate, sdp_mid, sdp_mline_index, .. } => {
                        let ice = webrtc::ice_transport::ice_candidate::RTCIceCandidateInit {
                            candidate,
                            sdp_mid,
                            sdp_mline_index: sdp_mline_index.map(|i| i as u16),
                            ..Default::default()
                        };
                        drop(sig);
                        peer_connection.add_ice_candidate(ice).await?;
                    }
                    _ => {}
                }
            }
        }
        
        Ok(Self {
            peer_connection,
            data_channel,
            rx,
            signaling,
            remote_peer_id: remote_peer_id.to_string(),
        })
    }
    
    /// Create a new WebRTC transport as the responder (callee)
    pub async fn new_as_callee(
        signaling_url: &str,
        local_peer_id: &str,
    ) -> Result<Self, anyhow::Error> {
        info!("Creating WebRTC transport as callee, peer ID: {}", local_peer_id);
        
        // Connect to signaling server
        let signaling = SignalingClient::connect(signaling_url, local_peer_id).await?;
        let signaling = Arc::new(Mutex::new(signaling));
        
        // Create WebRTC peer connection with STUN servers
        let config = RTCConfiguration {
            ice_servers: vec![
                RTCIceServer {
                    urls: vec![
                        "stun:stun.l.google.com:19302".to_string(),
                        "stun:stun1.l.google.com:19302".to_string(),
                    ],
                    ..Default::default()
                }
            ],
            ..Default::default()
        };
        
        let api = APIBuilder::new().build();
        let peer_connection = Arc::new(api.new_peer_connection(config).await?);
        
        // Channel for received messages
        let (tx, rx) = mpsc::channel::<Vec<u8>>(64);
        
        // Handle incoming data channels
        let tx_clone = tx.clone();
        let dc_holder: Arc<Mutex<Option<Arc<RTCDataChannel>>>> = Arc::new(Mutex::new(None));
        let dc_holder_clone = dc_holder.clone();
        
        peer_connection.on_data_channel(Box::new(move |dc| {
            let tx = tx_clone.clone();
            let holder = dc_holder_clone.clone();
            Box::pin(async move {
                info!("Data channel established: {}", dc.label());
                
                dc.on_message(Box::new(move |msg: DataChannelMessage| {
                    let tx = tx.clone();
                    Box::pin(async move {
                        let _ = tx.send(msg.data.to_vec()).await;
                    })
                }));
                
                let mut h = holder.lock().await;
                *h = Some(dc);
            })
        }));
        
        info!("Waiting for incoming offer...");
        
        let mut remote_peer_id = String::new();
        
        // Wait for offer from caller
        loop {
            let mut sig = signaling.lock().await;
            if let Some(msg) = sig.recv().await {
                match msg {
                    super::signaling::SignalMessage::Offer { peer_id, sdp } => {
                        info!("Received offer from {}", peer_id);
                        remote_peer_id = peer_id.clone();
                        
                        let offer = RTCSessionDescription::offer(sdp)?;
                        drop(sig);
                        peer_connection.set_remote_description(offer).await?;
                        
                        // Create and send answer
                        let answer = peer_connection.create_answer(None).await?;
                        peer_connection.set_local_description(answer.clone()).await?;
                        
                        let mut sig = signaling.lock().await;
                        sig.send_answer(&peer_id, &answer.sdp).await?;
                        break;
                    }
                    _ => {}
                }
            }
        }
        
        // Handle ICE candidates
        let signaling_ice = signaling.clone();
        let remote_peer = remote_peer_id.clone();
        peer_connection.on_ice_candidate(Box::new(move |candidate| {
            let signaling = signaling_ice.clone();
            let peer_id = remote_peer.clone();
            Box::pin(async move {
                if let Some(c) = candidate {
                    if let Ok(json) = c.to_json() {
                        let mut sig = signaling.lock().await;
                        let _ = sig.send_ice_candidate(
                            &peer_id,
                            &json.candidate,
                            json.sdp_mid,
                            json.sdp_mline_index,
                        ).await;
                    }
                }
            })
        }));
        
        // Wait for data channel to be ready
        let data_channel = loop {
            let h = dc_holder.lock().await;
            if let Some(dc) = h.clone() {
                break dc;
            }
            drop(h);
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        };
        
        Ok(Self {
            peer_connection,
            data_channel,
            rx,
            signaling,
            remote_peer_id,
        })
    }
}

#[async_trait]
impl Transport for WebRTCTransport {
    async fn send(&mut self, message: ProtocolMessage) -> Result<(), TransportError> {
        let data = bincode::serialize(&message)
            .map_err(|e| TransportError::SerializationError(format!("Serialize error: {}", e)))?;
        
        self.data_channel.send(&bytes::Bytes::from(data)).await
            .map_err(|e| TransportError::ProtocolError(format!("Send error: {}", e)))?;
        
        Ok(())
    }
    
    async fn receive(&mut self) -> Result<ProtocolMessage, TransportError> {
        let data = self.rx.recv().await
            .ok_or(TransportError::Closed)?;
        
        let message: ProtocolMessage = bincode::deserialize(&data)
            .map_err(|e| TransportError::SerializationError(format!("Deserialize error: {}", e)))?;
        
        Ok(message)
    }
    
    async fn close(&mut self) -> Result<(), TransportError> {
        self.peer_connection.close().await
            .map_err(|e| TransportError::ProtocolError(format!("Close error: {}", e)))?;
        Ok(())
    }
    
    fn is_connected(&self) -> bool {
        self.peer_connection.connection_state() == RTCPeerConnectionState::Connected
    }
}
