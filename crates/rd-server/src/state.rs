use std::sync::Arc;
use dashmap::DashMap;
use rd_core::domain::models::{SessionId, PeerId, Session};

/// Server state managing agents and sessions
#[derive(Clone)]
pub struct ServerState {
    /// Registered agents: device_id -> peer_id
    pub agents: Arc<DashMap<String, PeerId>>,
    
    /// Active sessions: session_id -> session
    pub sessions: Arc<DashMap<SessionId, Session>>,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(DashMap::new()),
            sessions: Arc::new(DashMap::new()),
        }
    }
    
    pub fn register_agent(&self, device_id: String, peer_id: PeerId) {
        self.agents.insert(device_id, peer_id);
    }
    
    pub fn get_agent(&self, device_id: &str) -> Option<PeerId> {
        self.agents.get(device_id).map(|entry| entry.value().clone())
    }
    
    pub fn add_session(&self, session: Session) {
        self.sessions.insert(session.id, session);
    }
    
    pub fn get_session(&self, session_id: SessionId) -> Option<Session> {
        self.sessions.get(&session_id).map(|entry| entry.value().clone())
    }
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}
