//! Machi - A Web3-native AI Agent Framework
//!
//! Machi provides a framework for building AI agents with embedded
//! cryptocurrency wallet capabilities. Each agent is created with its own
//! HD wallet identity, enabling autonomous blockchain interactions.
//!
//! # Features
//!
//! - **Native Wallet Identity**: Every agent has a built-in HD wallet
//! - **Multi-chain Support**: Ethereum, Solana, Bitcoin (via kobe)
//! - **Flexible Backends**: Support for rig, OpenAI, and more
//! - **Policy Control**: Fine-grained control over agent actions
//!
//! # Feature Flags
//!
//! - `rig` - Enable rig backend support
//! - `ethereum` - Enable Ethereum chain support
//!
//! # Example
//!
//! ```ignore
//! use machi::{Agent, AgentBuilder};
//! use machi::backend::rig::RigBackend;
//! use machi::chain::ethereum::Ethereum;
//!
//! #[tokio::main]
//! async fn main() -> machi::Result<()> {
//!     let agent = AgentBuilder::new()
//!         .backend(RigBackend::new(model))
//!         .chain(Ethereum::mainnet("https://eth.rpc.url"))
//!         .generate_wallet(12)?
//!         .build()?;
//!
//!     println!("Agent address: {}", agent.address()?);
//!     Ok(())
//! }
//! ```

pub mod agent;
pub mod backend;
pub mod chain;
pub mod error;
pub mod policy;
pub mod tools;
pub mod wallet;

// Re-exports for convenience
pub use agent::{Agent, AgentBuilder};
pub use error::{Error, Result};
pub use wallet::AgentWallet;
