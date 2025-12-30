use serde::{Serialize, Deserialize};
use rd_core::domain::models::*;
use rd_core::domain::ports::ProtocolMessage as CoreProtocolMessage;

/// Re-export core protocol message
pub use rd_core::domain::ports::ProtocolMessage;

/// Serialize a protocol message to bytes
pub fn serialize_message(msg: &ProtocolMessage) -> Result<Vec<u8>, bincode::Error> {
    bincode::serialize(msg)
}

/// Deserialize bytes to a protocol message
pub fn deserialize_message(data: &[u8]) -> Result<ProtocolMessage, bincode::Error> {
    bincode::deserialize(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_message_serialization() {
        let msg = ProtocolMessage::Heartbeat { timestamp: 12345 };
        
        let bytes = serialize_message(&msg).unwrap();
        assert!(!bytes.is_empty());
        
        let decoded = deserialize_message(&bytes).unwrap();
        match decoded {
            ProtocolMessage::Heartbeat { timestamp } => assert_eq!(timestamp, 12345),
            _ => panic!("Wrong message type"),
        }
    }
}
