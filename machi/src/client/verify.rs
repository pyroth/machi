use crate::{core::wasm_compat::WasmCompatSend, http};
use std::future::Future;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("invalid authentication")]
    InvalidAuthentication,
    #[error("provider error: {0}")]
    ProviderError(String),
    #[error("http error: {0}")]
    HttpError(#[from] http::Error),
}

/// A provider client that can verify the configuration.
pub trait VerifyClient {
    /// Verify the configuration.
    fn verify(&self) -> impl Future<Output = Result<(), VerifyError>> + WasmCompatSend;
}
