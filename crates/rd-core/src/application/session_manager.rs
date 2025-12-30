use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::Utc;

use crate::domain::{
    models::*,
    ports::*,
    error::*,
};

/// Session Manager - Orchestrates session lifecycle
pub struct SessionManager {
    repository: Arc<RwLock<dyn SessionRepository>>,
    authenticator: Arc<dyn Authenticator>,
}

impl SessionManager {
    pub fn new(
        repository: Arc<RwLock<dyn SessionRepository>>,
        authenticator: Arc<dyn Authenticator>,
    ) -> Self {
        Self {
            repository,
            authenticator,
        }
    }
    
    /// Create a new session between client and agent
    pub async fn create_session(
        &self,
        client_token: &AuthToken,
        agent_id: PeerId,
    ) -> Result<Session> {
        // Authenticate client
        let client_peer = self.authenticator
            .authenticate(client_token)
            .await?;
        
        // Create session
        let session = Session {
            id: SessionId::new(),
            client: client_peer,
            agent: agent_id,
            created_at: Utc::now(),
            status: SessionStatus::Pending,
        };
        
        // Store in repository
        let mut repo = self.repository.write().await;
        repo.create(session.clone()).await?;
        
        Ok(session)
    }
    
    /// Get session by ID
    pub async fn get_session(&self, id: SessionId) -> Result<Session> {
        let repo = self.repository.read().await;
        repo.find_by_id(id)
            .await?
            .ok_or(DomainError::SessionNotFound(id).into())
    }
    
    /// Update session status
    pub async fn update_status(&self, id: SessionId, status: SessionStatus) -> Result<()> {
        let mut session = self.get_session(id).await?;
        session.status = status;
        
        let mut repo = self.repository.write().await;
        repo.update(session).await?;
        
        Ok(())
    }
    
    /// End a session
    pub async fn end_session(&self, id: SessionId) -> Result<()> {
        self.update_status(id, SessionStatus::Closed).await?;
        
        let mut repo = self.repository.write().await;
        repo.delete(id).await?;
        
        Ok(())
    }
    
    /// List all active sessions
    pub async fn list_active_sessions(&self) -> Result<Vec<Session>> {
        let repo = self.repository.read().await;
        Ok(repo.list_active().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // TODO: Add tests with mock repository and authenticator
}
