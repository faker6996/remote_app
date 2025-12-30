//! WebRTC Transport Module
//! 
//! Provides P2P transport using WebRTC DataChannels for direct
//! peer-to-peer communication without relay server.

mod transport;
mod signaling;

pub use transport::WebRTCTransport;
pub use signaling::SignalingClient;
