//! Third-party platform integrations for Machi AI Agent Framework.
//!
//! This crate provides integrations with various platforms:
//! - Discord (feature: `discord`)
//! - Future: Telegram, Slack, databases, etc.

#[cfg(feature = "discord")]
#[cfg_attr(docsrs, doc(cfg(feature = "discord")))]
pub mod discord;

#[cfg(feature = "discord")]
pub use discord::DiscordExt;
