//! Machi Bot - A personal AI assistant framework with multi-channel support.
//!
//! This crate provides the infrastructure for building AI-powered chat bots
//! that can communicate through multiple channels (Telegram, WhatsApp, CLI, etc.).
//!
//! # Architecture
//!
//! The bot framework is organized around these core components:
//!
//! - **Message Bus** ([`bus`]) - Async pub-sub for channel-agent communication
//! - **Channels** ([`channels`]) - Platform integrations (CLI, Telegram, etc.)
//! - **Agent** ([`agent`]) - LLM-powered message processing loop
//! - **Gateway** ([`gateway`]) - Unified orchestration of all components
//! - **Session** ([`session`]) - Conversation state persistence
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use machi_bot::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let bus = MessageBus::new();
//!     let cli = CliChannel::new();
//!     cli.start(&bus).await?;
//!     Ok(())
//! }
//! ```
//!
//! # Features
//!
//! - `telegram` - Enable Telegram bot support via teloxide

// Core modules
pub mod agent;
pub mod bus;
pub mod channel;
pub mod channels;
pub mod config;
pub mod error;
pub mod events;
pub mod gateway;
pub mod session;
pub mod util;

// Optional/extended modules
pub mod cron;
pub mod heartbeat;
pub mod skills;
pub mod transcription;

/// Prelude module for convenient imports.
pub mod prelude {
    // Error types (centralized)
    pub use crate::error::{
        AgentError, AgentResult, BotError, BusError, BusResult, ChannelError, ChannelResult,
        ConfigError, ConfigResult, ErrorContext, Result, StorageError, StorageResult,
    };

    // Agent
    pub use crate::agent::{
        AgentLoop, AgentLoopBuilder, AgentLoopConfig, ContextBuilder, MessageRole,
    };

    // Bus
    pub use crate::bus::{InboundHandle, MessageBus, MessageBusBuilder, OutboundHandle};

    // Channel
    pub use crate::channel::{
        AllowlistConfig, BoxedChannel, Channel, ChannelBase, ChannelManager, ChannelState,
        ChannelStatus,
    };
    pub use crate::channels::CliChannel;
    #[cfg(feature = "telegram")]
    pub use crate::channels::{TelegramChannel, telegram::TelegramChannelConfig};

    // Config
    pub use crate::config::{
        BotConfig, ChannelConfig, ConfigIssue, ExecConfig, GeminiConfig, GroqConfig, IssueLevel,
        ProviderConfig, TelegramConfig, ToolsConfig, VllmConfig, config_path, init_config,
        load_config, save_config,
    };

    // Events
    pub use crate::events::{
        InboundMessage, MediaAttachment, MediaType, MessageFormat, OutboundMessage,
    };

    // Gateway
    pub use crate::gateway::{Gateway, GatewayBuilder, GatewayConfig, GatewayStatus};

    // Session
    pub use crate::session::{FileStorage, MemoryStorage, Session, SessionManager, SessionStorage};

    // Utilities
    pub use crate::util::{
        config_dir, config_path as util_config_path, generate_id, generate_message_id, home_dir,
        sessions_dir, split_into_chunks, timestamp_ms, truncate_str, workspace_dir,
    };

    // Cron (re-export for convenience)
    pub use crate::cron::{
        CronJob, CronJobBuilder, CronJobId, CronSchedule, CronScheduler, CronStorage,
        FileCronStorage, JobStatus, MemoryCronStorage, SchedulerHandle,
    };

    // Heartbeat
    pub use crate::heartbeat::{HealthStatus, HeartbeatConfig, HeartbeatHandle, HeartbeatService};

    // Skills
    pub use crate::skills::{Skill, SkillInfo, SkillLoader, SkillManifest, SkillRegistry};

    // Transcription
    pub use crate::transcription::{GroqTranscriber, TranscribeResult, Transcriber};
}
