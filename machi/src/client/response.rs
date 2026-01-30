//! Response types for client operations.

use serde::{Deserialize, Serialize};

use crate::completion::{GetTokenUsage, Usage};

/// The final streaming response from a dynamic client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalCompletionResponse {
    /// Token usage information, if available.
    pub usage: Option<Usage>,
}

impl GetTokenUsage for FinalCompletionResponse {
    fn token_usage(&self) -> Option<Usage> {
        self.usage
    }
}
