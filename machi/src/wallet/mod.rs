//! Wallet module for AI agent blockchain interactions.
//!
//! This module provides wallet capabilities that allow machi agents to
//! autonomously interact with blockchains — reading on-chain data, signing
//! messages, and submitting transactions.
//!
//! # Architecture
//!
//! ```text
//! EvmWallet (kobe HD + alloy signer + alloy provider)
//!   ├── builder()     → EvmWalletBuilder → build()
//!   ├── balance()     → query ETH balance
//!   ├── transfer()    → send ETH
//!   ├── sign_message()→ EIP-191 personal sign
//!   └── tools()       → Vec<BoxedTool> for Agent integration
//! ```
//!
//! # Key Derivation
//!
//! Uses [`kobe`] for BIP39 mnemonic management and [`kobe_eth`] for
//! Ethereum HD key derivation (BIP32/44). The derived private key is
//! then used with [`alloy`]'s `PrivateKeySigner` for signing.
//!
//! # Examples
//!
//! ```rust,ignore
//! use machi::wallet::EvmWallet;
//!
//! // Create wallet from mnemonic
//! let wallet = EvmWallet::builder()
//!     .mnemonic("abandon abandon abandon ...")
//!     .rpc_url("https://eth-mainnet.g.alchemy.com/v2/xxx")
//!     .build()
//!     .await?;
//!
//! // Check balance
//! let balance = wallet.balance().await?;
//!
//! // Use as agent tools
//! let agent = Agent::new("defi-bot")
//!     .tools(wallet.tools())
//!     .provider(llm_provider);
//! ```

mod error;
mod evm;
#[allow(clippy::unnecessary_literal_bound)]
pub(crate) mod tools;

pub use error::WalletError;
pub use evm::{EvmWallet, EvmWalletBuilder};
