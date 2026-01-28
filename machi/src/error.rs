//! Unified error types for machi.

use thiserror::Error;

/// The main error type for machi operations.
#[derive(Debug, Error)]
pub enum Error {
    /// Wallet-related errors.
    #[error("wallet error: {0}")]
    Wallet(String),

    /// Backend (LLM) errors.
    #[error("backend error: {0}")]
    Backend(String),

    /// Chain operation errors.
    #[error("chain error: {0}")]
    Chain(String),

    /// Policy violation errors.
    #[error("policy violation: {0}")]
    Policy(String),

    /// Configuration errors.
    #[error("config error: {0}")]
    Config(String),

    /// Tool execution errors.
    #[error("tool error: {0}")]
    Tool(String),

    /// Kobe wallet errors.
    #[error("kobe error: {0}")]
    Kobe(#[from] kobe_core::Error),

    /// Kobe ETH errors.
    #[error("kobe-eth error: {0}")]
    KobeEth(#[from] kobe_eth::Error),

    /// JSON serialization errors.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Convenient Result type alias.
pub type Result<T> = std::result::Result<T, Error>;
