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
    data_channel::{
        data_channel_message::DataChannelMessage,
        data_channel_state::RTCDataChannelState,
        RTCDataChannel,
    },
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
#[derive(Clone)]
pub struct WebRTCTransport {
    peer_connection: Arc<RTCPeerConnection>,
    data_channel: Arc<RTCDataChannel>,
    rx: Arc<Mutex<mpsc::Receiver<Vec<u8>>>>,
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
        
        // Wait for answer from signaling and forward any early ICE candidates
        let mut got_answer = false;
        let start_wait = tokio::time::Instant::now();
        while !got_answer {
            if start_wait.elapsed() > tokio::time::Duration::from_secs(15) {
                return Err(anyhow::anyhow!("Timeout waiting for SDP answer from {}", remote_peer_id));
            }
            let mut sig = signaling.lock().await;
            if let Some(msg) = sig.recv().await {
                match msg {
                    super::signaling::SignalMessage::Answer { sdp, .. } => {
                        info!("Received answer from peer");
                        let answer = RTCSessionDescription::answer(sdp)?;
                        drop(sig);
                        peer_connection.set_remote_description(answer).await?;
                        got_answer = true;
                    }
                    super::signaling::SignalMessage::IceCandidate { candidate, sdp_mid, sdp_mline_index, .. } => {
                        let ice = webrtc::ice_transport::ice_candidate::RTCIceCandidateInit {
                            candidate,
                            sdp_mid,
                            sdp_mline_index: sdp_mline_index.map(|i| i as u16),
                            ..Default::default()
                        };
                        drop(sig);
                        let _ = peer_connection.add_ice_candidate(ice).await;
                    }
                    _ => {}
                }
            } else {
                drop(sig);
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
        }
        
        // Spawn background task to continuously receive any late ICE candidates from signaling
        let pc_ice = peer_connection.clone();
        let sig_ice = signaling.clone();
        tokio::spawn(async move {
            loop {
                let mut sig = sig_ice.lock().await;
                match sig.recv().await {
                    Some(super::signaling::SignalMessage::IceCandidate { candidate, sdp_mid, sdp_mline_index, .. }) => {
                        let ice = webrtc::ice_transport::ice_candidate::RTCIceCandidateInit {
                            candidate,
                            sdp_mid,
                            sdp_mline_index: sdp_mline_index.map(|i| i as u16),
                            ..Default::default()
                        };
                        drop(sig);
                        let _ = pc_ice.add_ice_candidate(ice).await;
                    }
                    Some(_) => {}
                    None => break,
                }
            }
        });

        // Wait until DataChannel reaches Open state!
        info!("Waiting for caller DataChannel to open...");
        let open_timeout = tokio::time::Instant::now();
        while data_channel.ready_state() != RTCDataChannelState::Open {
            if open_timeout.elapsed() > tokio::time::Duration::from_secs(15) {
                return Err(anyhow::anyhow!("Timeout waiting for DataChannel to reach Open state (currently {:?})", data_channel.ready_state()));
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
        info!("Caller DataChannel is now OPEN!");

        Ok(Self {
            peer_connection,
            data_channel,
            rx: Arc::new(Mutex::new(rx)),
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
        let mut got_offer = false;
        while !got_offer {
            let mut sig = signaling.lock().await;
            if let Some(msg) = sig.recv().await {
                match msg {
                    super::signaling::SignalMessage::Offer { peer_id, sdp } => {
                        info!("Received offer from {}", peer_id);
                        remote_peer_id = peer_id.clone();
                        
                        // Handle local ICE candidates and forward to caller
                        let signaling_ice = signaling.clone();
                        let peer_id_for_ice = peer_id.clone();
                        peer_connection.on_ice_candidate(Box::new(move |candidate| {
                            let signaling = signaling_ice.clone();
                            let peer_id = peer_id_for_ice.clone();
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

                        let offer = RTCSessionDescription::offer(sdp)?;
                        drop(sig);
                        peer_connection.set_remote_description(offer).await?;
                        
                        // Create and send answer
                        let answer = peer_connection.create_answer(None).await?;
                        peer_connection.set_local_description(answer.clone()).await?;
                        
                        let mut sig = signaling.lock().await;
                        sig.send_answer(&peer_id, &answer.sdp).await?;
                        got_offer = true;
                    }
                    super::signaling::SignalMessage::IceCandidate { candidate, sdp_mid, sdp_mline_index, .. } => {
                        let ice = webrtc::ice_transport::ice_candidate::RTCIceCandidateInit {
                            candidate,
                            sdp_mid,
                            sdp_mline_index: sdp_mline_index.map(|i| i as u16),
                            ..Default::default()
                        };
                        drop(sig);
                        let _ = peer_connection.add_ice_candidate(ice).await;
                    }
                    _ => {}
                }
            } else {
                drop(sig);
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
        }
        
        // Spawn background task to continuously receive any late ICE candidates from caller
        let pc_ice = peer_connection.clone();
        let sig_ice = signaling.clone();
        tokio::spawn(async move {
            loop {
                let mut sig = sig_ice.lock().await;
                match sig.recv().await {
                    Some(super::signaling::SignalMessage::IceCandidate { candidate, sdp_mid, sdp_mline_index, .. }) => {
                        let ice = webrtc::ice_transport::ice_candidate::RTCIceCandidateInit {
                            candidate,
                            sdp_mid,
                            sdp_mline_index: sdp_mline_index.map(|i| i as u16),
                            ..Default::default()
                        };
                        drop(sig);
                        let _ = pc_ice.add_ice_candidate(ice).await;
                    }
                    Some(_) => {}
                    None => break,
                }
            }
        });

        // Wait for data channel to be ready from on_data_channel
        let wait_dc_start = tokio::time::Instant::now();
        let data_channel = loop {
            let h = dc_holder.lock().await;
            if let Some(dc) = h.clone() {
                break dc;
            }
            drop(h);
            if wait_dc_start.elapsed() > tokio::time::Duration::from_secs(15) {
                return Err(anyhow::anyhow!("Timeout waiting for on_data_channel from caller"));
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        };

        // Wait until Callee DataChannel reaches Open state!
        info!("Waiting for Callee DataChannel to open...");
        let open_timeout = tokio::time::Instant::now();
        while data_channel.ready_state() != RTCDataChannelState::Open {
            if open_timeout.elapsed() > tokio::time::Duration::from_secs(15) {
                return Err(anyhow::anyhow!("Timeout waiting for Callee DataChannel to open (currently {:?})", data_channel.ready_state()));
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
        info!("Callee DataChannel is now OPEN!");
        
        Ok(Self {
            peer_connection,
            data_channel,
            rx: Arc::new(Mutex::new(rx)),
            signaling,
            remote_peer_id,
        })
    }

    /// Send a protocol message over the WebRTC data channel (non-blocking to receiver)
    pub async fn send_msg(&self, message: ProtocolMessage) -> Result<(), TransportError> {
        let data = bincode::serialize(&message)
            .map_err(|e| TransportError::SerializationError(format!("Serialize error: {}", e)))?;
        
        let start = tokio::time::Instant::now();
        while self.data_channel.ready_state() != RTCDataChannelState::Open {
            if start.elapsed() > tokio::time::Duration::from_secs(5) {
                return Err(TransportError::ProtocolError("DataChannel is not opened".to_string()));
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        self.data_channel.send(&bytes::Bytes::from(data)).await
            .map_err(|e| TransportError::ProtocolError(format!("Send error: {}", e)))?;
        
        Ok(())
    }
    
    /// Receive a protocol message from the WebRTC data channel
    pub async fn recv_msg(&self) -> Result<ProtocolMessage, TransportError> {
        let mut rx = self.rx.lock().await;
        let data = rx.recv().await
            .ok_or(TransportError::Closed)?;
        
        let message: ProtocolMessage = bincode::deserialize(&data)
            .map_err(|e| TransportError::SerializationError(format!("Deserialize error: {}", e)))?;
        
        Ok(message)
    }

    /// Check if peer connection is active
    pub fn is_connected(&self) -> bool {
        self.peer_connection.connection_state() == RTCPeerConnectionState::Connected
    }

    /// Close the peer connection
    pub async fn close(&self) -> Result<(), TransportError> {
        self.peer_connection.close().await
            .map_err(|e| TransportError::ProtocolError(format!("Close error: {}", e)))?;
        Ok(())
    }

    pub fn remote_peer_id(&self) -> &str {
        &self.remote_peer_id
    }
}

#[async_trait]
impl Transport for WebRTCTransport {
    async fn send(&mut self, message: ProtocolMessage) -> Result<(), TransportError> {
        self.send_msg(message).await
    }
    
    async fn receive(&mut self) -> Result<ProtocolMessage, TransportError> {
        self.recv_msg().await
    }
    
    async fn close(&mut self) -> Result<(), TransportError> {
        WebRTCTransport::close(self).await
    }
    
    fn is_connected(&self) -> bool {
        WebRTCTransport::is_connected(self)
    }
}
