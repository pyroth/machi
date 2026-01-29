use crate::core::wasm_compat::WasmCompatSend;
use std::future::Future;

use super::errors::VerifyError;

/// A provider client that can verify the configuration.
pub trait VerifyClient {
    /// Verify the configuration.
    fn verify(&self) -> impl Future<Output = Result<(), VerifyError>> + WasmCompatSend;
}
