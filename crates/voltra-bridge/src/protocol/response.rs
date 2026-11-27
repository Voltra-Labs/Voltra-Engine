//! What the editor answers a request with.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The answer to the request of the same id: a JSON value, or why not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    /// The id of the request it answers.
    pub id: u64,
    /// The result, when the command succeeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ok: Option<Value>,
    /// Why the command failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    /// The response to request `id` for the outcome `result`.
    pub fn answer(id: u64, result: Result<Value, String>) -> Self {
        match result {
            Ok(value) => Self {
                id,
                ok: Some(value),
                error: None,
            },
            Err(error) => Self {
                id,
                ok: None,
                error: Some(error),
            },
        }
    }

    /// The result: the error if there is one, else the value, or null if there is neither.
    pub fn into_result(self) -> Result<Value, String> {
        match (self.ok, self.error) {
            (_, Some(error)) => Err(error),
            (Some(value), None) => Ok(value),
            (None, None) => Ok(Value::Null),
        }
    }
}
