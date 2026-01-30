//! Verify client trait.

use std::future::Future;

use crate::core::wasm_compat::WasmCompatSend;

use super::super::error::VerifyError;

/// A provider client that can verify its configuration.
pub trait VerifyClient {
    /// Verifies the client configuration (e.g., API key validity).
    fn verify(&self) -> impl Future<Output = Result<(), VerifyError>> + WasmCompatSend;
}
