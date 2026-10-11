use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::LldbMcpError,
    session::{SessionId, SessionInfo, SessionStore, create_session, delete_session, list_sessions},
};

/// Parameters for creating a debug session.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SessionCreateParams {
    /// Human-readable label for the session.
    pub name: String,
}

/// Result of creating a debug session.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SessionCreateResult {
    pub session_id: String,
}

/// Parameters for deleting a debug session.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SessionDeleteParams {
    /// The ID of the session to delete.
    pub session_id: String,
}

pub async fn handle_session_create(
    store: &SessionStore,
    params: SessionCreateParams,
) -> Result<SessionCreateResult, LldbMcpError> {
    let session_id = create_session(store, params.name).await?;
    Ok(SessionCreateResult {
        session_id: session_id.to_string(),
    })
}

pub async fn handle_session_list(store: &SessionStore) -> Vec<SessionInfo> {
    list_sessions(store).await
}

pub async fn handle_session_delete(
    store: &SessionStore,
    params: SessionDeleteParams,
) -> Result<String, LldbMcpError> {
    let uuid = Uuid::parse_str(&params.session_id)
        .map_err(|_| LldbMcpError::InvalidArgument(format!("Invalid UUID: {}", params.session_id)))?;
    let session_id = SessionId(uuid);
    delete_session(store, &session_id).await?;
    Ok(format!("Session {} deleted successfully", params.session_id))
}
