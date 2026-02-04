//! Channel implementations for various messaging platforms.
//!
//! This module provides concrete implementations of the [`Channel`] trait
//! for different messaging platforms.
//!
//! # Available Channels
//!
//! - [`cli::CliChannel`] - Command-line interface channel (always available)
//! - [`telegram::TelegramChannel`] - Telegram bot (requires `telegram` feature)
//!
//! # Feature Flags
//!
//! - `telegram` - Enable Telegram support via teloxide

pub mod cli;

#[cfg(feature = "telegram")]
pub mod telegram;

pub use cli::CliChannel;

#[cfg(feature = "telegram")]
pub use telegram::TelegramChannel;
