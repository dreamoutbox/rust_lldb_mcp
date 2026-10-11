use std::time::SystemTime;

use crate::{
    driver::LldbProcess,
    error::LldbMcpError,
    session::store::{DebugSession, SessionId, SessionInfo, SessionStore},
};

/// Creates a new debug session by spawning an LLDB process.
pub async fn create_session(store: &SessionStore, name: String) -> Result<SessionId, LldbMcpError> {
    let process = LldbProcess::spawn().await?;
    let id = SessionId::new();

    let session = DebugSession {
        id,
        name,
        created_at: SystemTime::now(),
        process,
        target_executable: None,
    };

    let mut map = store.lock().await;
    map.insert(id, session);

    Ok(id)
}

/// Lists all currently active debug sessions.
pub async fn list_sessions(store: &SessionStore) -> Vec<SessionInfo> {
    let map = store.lock().await;
    map.values().map(DebugSession::info).collect()
}

/// Terminates the LLDB subprocess and removes the session from the store.
pub async fn delete_session(store: &SessionStore, id: &SessionId) -> Result<(), LldbMcpError> {
    let mut session = {
        let mut map = store.lock().await;
        map.remove(id)
            .ok_or_else(|| LldbMcpError::SessionNotFound(id.to_string()))?
    };

    session.process.terminate().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_lifecycle() {
        let store = SessionStore::new();

        let session_id = create_session(&store, "test_session".to_string())
            .await
            .expect("Failed to create session");

        let sessions = list_sessions(&store).await;
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, "test_session");
        assert_eq!(sessions[0].session_id, session_id.to_string());

        delete_session(&store, &session_id)
            .await
            .expect("Failed to delete session");

        let sessions_after = list_sessions(&store).await;
        assert!(sessions_after.is_empty());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_session() {
        let store = SessionStore::new();
        let invalid_id = SessionId::new();

        let result = delete_session(&store, &invalid_id).await;
        assert!(result.is_err());
    }
}
