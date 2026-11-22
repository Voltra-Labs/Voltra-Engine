//! What each tool does, apart from the protocol: plain functions over a
//! [`Project`](crate::Project), answering JSON or an error a client can
//! read and act on.

pub mod discovery;
pub mod entity_ref;
pub mod scene_edit;
pub mod scene_read;

use std::fmt;

/// A tool that could not do what it was asked, in words for the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolError(pub String);

impl ToolError {
    /// The error any engine or IO failure `cause` names, as the client
    /// reads it.
    pub fn from_cause(cause: impl fmt::Display) -> Self {
        Self(cause.to_string())
    }
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ToolError {}

/// What a tool answers.
pub type ToolResult = Result<serde_json::Value, ToolError>;
