use async_trait::async_trait;
use quinn::{Connection, SendStream, RecvStream};
use bytes::{Bytes, BytesMut, BufMut};
use tracing::{debug, warn, error};

use rd_core::domain::{
    ports::{Transport, ProtocolMessage},
    error::TransportError,
};

use crate::protocol::{serialize_message, deserialize_message};

/// QUIC transport implementation
pub struct QuicTransport {
    connection: Connection,
    send_stream: Option<SendStream>,
    recv_stream: Option<RecvStream>,
}

impl QuicTransport {
    /// Create a new QUIC transport from an established connection
    pub async fn new(connection: Connection) -> Result<Self, TransportError> {
        Ok(Self {
            connection,
            send_stream: None,
            recv_stream: None,
        })
    }
    
    /// Initialize bidirectional stream
    async fn ensure_stream(&mut self) -> Result<(), TransportError> {
        if self.send_stream.is_none() || self.recv_stream.is_none() {
            let (send, recv) = self.connection
                .open_bi()
                .await
                .map_err(|e| TransportError::ConnectionFailed(e.to_string()))?;
            
            self.send_stream = Some(send);
            self.recv_stream = Some(recv);
            
            debug!("Opened bidirectional QUIC stream");
        }
        Ok(())
    }
}

#[async_trait]
impl Transport for QuicTransport {
    async fn send(&mut self, message: ProtocolMessage) -> Result<(), TransportError> {
        self.ensure_stream().await?;
        
        // Serialize message
        let data = serialize_message(&message)
            .map_err(|e| TransportError::SerializationError(e.to_string()))?;
        
        // Send length prefix (4 bytes)
        let len = data.len() as u32;
        let mut frame = BytesMut::with_capacity(4 + data.len());
        frame.put_u32(len);
        frame.put_slice(&data);
        
        // Send frame
        let send_stream = self.send_stream.as_mut().unwrap();
        send_stream
            .write_all(&frame)
            .await
            .map_err(|_| TransportError::Closed)?;
        
        debug!("Sent message ({} bytes)", data.len());
        
        Ok(())
    }
    
    async fn receive(&mut self) -> Result<ProtocolMessage, TransportError> {
        self.ensure_stream().await?;
        
        let recv_stream = self.recv_stream.as_mut().unwrap();
        
        // Read length prefix (4 bytes)
        let mut len_buf = [0u8; 4];
        recv_stream
            .read_exact(&mut len_buf)
            .await
            .map_err(|e| match e {
                quinn::ReadExactError::FinishedEarly(_) => TransportError::Closed,
                quinn::ReadExactError::ReadError(e) => TransportError::IoError(e.into()),
            })?;
        
        let len = u32::from_be_bytes(len_buf) as usize;
        
        // Read message data
        let mut data = vec![0u8; len];
        recv_stream
            .read_exact(&mut data)
            .await
            .map_err(|e| match e {
                quinn::ReadExactError::FinishedEarly(_) => TransportError::Closed,
                quinn::ReadExactError::ReadError(e) => TransportError::IoError(e.into()),
            })?;
        
        // Deserialize message
        let message = deserialize_message(&data)
            .map_err(|e| TransportError::SerializationError(e.to_string()))?;
        
        debug!("Received message ({} bytes)", len);
        
        Ok(message)
    }
    
    async fn close(&mut self) -> Result<(), TransportError> {
        if let Some(mut send) = self.send_stream.take() {
            send.finish()
                .map_err(|e| TransportError::IoError(e.into()))?;
        }
        
        self.connection.close(0u32.into(), b"closing");
        
        debug!("Closed QUIC transport");
        
        Ok(())
    }
    
    fn is_connected(&self) -> bool {
        !self.connection.close_reason().is_some()
    }
}
