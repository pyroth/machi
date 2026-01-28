//! Wallet wrapper for agent identity.
//!
//! This module provides a thin wrapper around kobe's Wallet type,
//! adding agent-specific functionality.

use kobe_core::Wallet;

use crate::error::Result;

/// An agent's wallet identity.
///
/// This wraps a kobe Wallet and provides a unified interface for
/// deriving addresses across multiple chains.
#[derive(Debug)]
pub struct AgentWallet {
    /// The underlying kobe wallet.
    inner: Wallet,
    /// Default derivation index.
    default_index: u32,
}

impl AgentWallet {
    /// Create a wallet from a mnemonic phrase.
    pub fn from_mnemonic(phrase: &str, passphrase: Option<&str>) -> Result<Self> {
        let inner = Wallet::from_mnemonic(phrase, passphrase)?;
        Ok(Self {
            inner,
            default_index: 0,
        })
    }

    /// Generate a new random wallet.
    ///
    /// # Arguments
    ///
    /// * `word_count` - Number of mnemonic words (12, 15, 18, 21, or 24)
    /// * `passphrase` - Optional BIP39 passphrase
    pub fn generate(word_count: usize, passphrase: Option<&str>) -> Result<Self> {
        let inner = Wallet::generate(word_count, passphrase)?;
        Ok(Self {
            inner,
            default_index: 0,
        })
    }

    /// Get the seed bytes for key derivation.
    #[inline]
    pub fn seed(&self) -> &[u8; 64] {
        self.inner.seed()
    }

    /// Get the mnemonic phrase.
    ///
    /// **Security Warning**: Handle this value carefully.
    #[inline]
    pub fn mnemonic(&self) -> &str {
        self.inner.mnemonic()
    }

    /// Get the default derivation index.
    #[inline]
    pub const fn default_index(&self) -> u32 {
        self.default_index
    }

    /// Set the default derivation index.
    #[inline]
    pub fn set_default_index(&mut self, index: u32) {
        self.default_index = index;
    }

    /// Get the underlying kobe wallet.
    #[inline]
    pub const fn inner(&self) -> &Wallet {
        &self.inner
    }
}
