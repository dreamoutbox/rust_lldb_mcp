use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc, time::SystemTime};
use tokio::sync::{Mutex, MutexGuard};
use uuid::Uuid;

use crate::driver::LldbProcess;

/// Strongly typed session identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[schemars(transparent)]
pub struct SessionId(#[schemars(with = "String")] pub Uuid);

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

/// Metadata summary of a debug session.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionInfo {
    pub session_id: String,
    pub name: String,
    pub target_executable: Option<String>,
}

/// Active LLDB debug session with associated subprocess.
pub struct DebugSession {
    pub id: SessionId,
    pub name: String,
    pub created_at: SystemTime,
    pub process: LldbProcess,
    pub target_executable: Option<String>,
}

impl DebugSession {
    pub fn info(&self) -> SessionInfo {
        SessionInfo {
            session_id: self.id.to_string(),
            name: self.name.clone(),
            target_executable: self.target_executable.clone(),
        }
    }
}

/// Shared in-memory storage of all active debug sessions.
#[derive(Clone, Default)]
pub struct SessionStore {
    sessions: Arc<Mutex<HashMap<SessionId, DebugSession>>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn lock(&self) -> MutexGuard<'_, HashMap<SessionId, DebugSession>> {
        self.sessions.lock().await
    }
}

/// Parses a string into a SessionId.
pub fn parse_session_id(id_str: &str) -> Result<SessionId, crate::error::LldbMcpError> {
    let uuid = Uuid::parse_str(id_str).map_err(|_| {
        crate::error::LldbMcpError::InvalidArgument(format!("Invalid UUID: {id_str}"))
    })?;
    Ok(SessionId(uuid))
}
