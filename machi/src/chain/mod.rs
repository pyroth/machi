//! Blockchain adapters for wallet operations.
//!
//! This module defines the [`Chain`] trait that abstracts over different
//! blockchain networks (Ethereum, Solana, Bitcoin, etc.).
//!
//! # Supported Chains
//!
//! - Ethereum (feature = "ethereum")

use std::future::Future;

use kobe_core::Wallet;

use crate::error::Result;

#[cfg(feature = "ethereum")]
pub mod ethereum;

/// Transaction hash representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxHash(pub String);

impl std::fmt::Display for TxHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A blockchain transaction request.
#[derive(Debug, Clone)]
pub struct TransactionRequest {
    /// Recipient address.
    pub to: String,
    /// Amount in the chain's smallest unit (wei for ETH, lamports for SOL).
    pub value: u128,
    /// Optional calldata for contract interactions.
    pub data: Option<Vec<u8>>,
}

/// Trait for blockchain adapters.
///
/// Implement this trait to add support for new blockchain networks.
/// Each implementation should use the corresponding kobe chain module.
pub trait Chain: Send + Sync {
    /// The address type for this chain.
    type Address: AsRef<str> + Send;

    /// Get the chain name (e.g., "ethereum", "solana").
    fn name(&self) -> &'static str;

    /// Derive an address from a wallet at the given index.
    fn derive_address(&self, wallet: &Wallet, index: u32) -> Result<Self::Address>;

    /// Get the balance of an address.
    fn balance(&self, address: &str) -> impl Future<Output = Result<u128>> + Send;

    /// Send a transaction.
    fn send_transaction(
        &self,
        wallet: &Wallet,
        index: u32,
        tx: TransactionRequest,
    ) -> impl Future<Output = Result<TxHash>> + Send;
}
