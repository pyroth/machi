//! Telegram channel implementation using teloxide.
//!
//! This module provides a Telegram bot integration that can receive and send
//! messages through the Telegram Bot API.
//!
//! # Setup
//!
//! 1. Create a bot via [@BotFather](https://t.me/botfather)
//! 2. Get your bot token
//! 3. Configure the channel with the token
//!
//! # Example
//!
//! ```rust,ignore
//! use machi_bot::channels::TelegramChannel;
//! use machi_bot::bus::MessageBus;
//!
//! let config = TelegramChannelConfig::new("YOUR_BOT_TOKEN")
//!     .allow_user("123456789");
//!
//! let bus = MessageBus::new();
//! let telegram = TelegramChannel::new(config);
//! telegram.start(&bus).await?;
//! ```

use crate::agent::confirmation::{
    ConfirmationManager, ConfirmationRequest, TelegramConfirmationHandler,
};
use crate::bus::MessageBus;
use crate::channel::{Channel, ChannelBase, ChannelState, ChannelStatus};
use crate::error::{ChannelError, ChannelResult};
use crate::events::{InboundMessage, MessageFormat, OutboundMessage};
use async_trait::async_trait;
use regex::Regex;
use std::sync::{Arc, OnceLock};
use teloxide::prelude::*;
use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardMarkup, MediaKind, MessageKind, ParseMode,
};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info};

/// Telegram channel configuration.
#[derive(Debug, Clone)]
pub struct TelegramChannelConfig {
    /// Bot token from @BotFather.
    pub token: String,
    /// Allowed user IDs. Empty means allow all (not recommended).
    pub allowed_users: Vec<i64>,
    /// Allowed chat IDs. Empty means allow all.
    pub allowed_chats: Vec<i64>,
    /// Whether to parse messages as markdown.
    pub parse_markdown: bool,
    /// Maximum message length before splitting.
    pub max_message_length: usize,
}

impl TelegramChannelConfig {
    /// Create a new Telegram channel config with the given token.
    #[must_use]
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            allowed_users: Vec::new(),
            allowed_chats: Vec::new(),
            parse_markdown: true,
            max_message_length: 4096, // Telegram's limit
        }
    }

    /// Create config from environment variable `TELEGRAM_BOT_TOKEN`.
    ///
    /// # Panics
    ///
    /// Panics if `TELEGRAM_BOT_TOKEN` is not set.
    #[must_use]
    pub fn from_env() -> Self {
        let token = std::env::var("TELEGRAM_BOT_TOKEN")
            .expect("TELEGRAM_BOT_TOKEN environment variable not set");
        Self::new(token)
    }

    /// Try to create config from environment variable.
    #[must_use]
    pub fn try_from_env() -> Option<Self> {
        std::env::var("TELEGRAM_BOT_TOKEN").ok().map(Self::new)
    }

    /// Add an allowed user ID.
    #[must_use]
    pub fn allow_user(mut self, user_id: i64) -> Self {
        self.allowed_users.push(user_id);
        self
    }

    /// Add multiple allowed user IDs.
    #[must_use]
    pub fn allow_users(mut self, user_ids: impl IntoIterator<Item = i64>) -> Self {
        self.allowed_users.extend(user_ids);
        self
    }

    /// Add an allowed chat ID.
    #[must_use]
    pub fn allow_chat(mut self, chat_id: i64) -> Self {
        self.allowed_chats.push(chat_id);
        self
    }

    /// Set whether to parse messages as markdown.
    #[must_use]
    pub const fn parse_markdown(mut self, enabled: bool) -> Self {
        self.parse_markdown = enabled;
        self
    }

    /// Check if a user is allowed.
    #[must_use]
    pub fn is_user_allowed(&self, user_id: i64) -> bool {
        self.allowed_users.is_empty() || self.allowed_users.contains(&user_id)
    }

    /// Check if a chat is allowed.
    #[must_use]
    pub fn is_chat_allowed(&self, chat_id: i64) -> bool {
        self.allowed_chats.is_empty() || self.allowed_chats.contains(&chat_id)
    }
}

/// Telegram channel implementation.
pub struct TelegramChannel {
    base: ChannelBase,
    config: TelegramChannelConfig,
    bot: RwLock<Option<Bot>>,
    shutdown_tx: RwLock<Option<mpsc::Sender<()>>>,
    /// Confirmation manager for tool execution approval.
    confirmation_manager: Arc<ConfirmationManager>,
}

impl std::fmt::Debug for TelegramChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TelegramChannel")
            .field("base", &self.base)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl TelegramChannel {
    /// Create a new Telegram channel with the given configuration.
    #[must_use]
    pub fn new(config: TelegramChannelConfig) -> Self {
        Self {
            base: ChannelBase::new("telegram"),
            config,
            bot: RwLock::new(None),
            shutdown_tx: RwLock::new(None),
            confirmation_manager: Arc::new(ConfirmationManager::new()),
        }
    }

    /// Create a new Telegram channel with a shared confirmation manager.
    #[must_use]
    pub fn with_confirmation_manager(
        config: TelegramChannelConfig,
        manager: Arc<ConfirmationManager>,
    ) -> Self {
        Self {
            base: ChannelBase::new("telegram"),
            config,
            bot: RwLock::new(None),
            shutdown_tx: RwLock::new(None),
            confirmation_manager: manager,
        }
    }

    /// Create a Telegram channel from environment variables.
    #[must_use]
    pub fn from_env() -> Self {
        Self::new(TelegramChannelConfig::from_env())
    }

    /// Get the confirmation handler for this channel.
    #[must_use]
    pub fn confirmation_handler(&self) -> TelegramConfirmationHandler {
        TelegramConfirmationHandler::new(Arc::clone(&self.confirmation_manager))
    }

    /// Get the confirmation manager.
    #[must_use]
    pub fn confirmation_manager(&self) -> &Arc<ConfirmationManager> {
        &self.confirmation_manager
    }

    /// Send a confirmation request with inline keyboard buttons.
    pub async fn send_confirmation_request(
        &self,
        chat_id: i64,
        request: &ConfirmationRequest,
    ) -> ChannelResult<()> {
        let bot = self.bot.read().await;
        let bot = bot.as_ref().ok_or(ChannelError::NotConnected)?;

        // Build inline keyboard
        let keyboard = InlineKeyboardMarkup::new(vec![vec![
            InlineKeyboardButton::callback("✅ Yes", format!("confirm:{}:y", request.id)),
            InlineKeyboardButton::callback("❌ No", format!("confirm:{}:n", request.id)),
            InlineKeyboardButton::callback("✅ All", format!("confirm:{}:a", request.id)),
        ]]);

        let message = format!(
            "🔐 <b>Tool Confirmation Required</b>\n\n{}",
            Self::markdown_to_telegram_html(&request.description)
        );

        bot.send_message(ChatId(chat_id), message)
            .parse_mode(ParseMode::Html)
            .reply_markup(keyboard)
            .await
            .map_err(|e| ChannelError::SendFailed(e.to_string()))?;

        Ok(())
    }

    /// Convert Markdown to Telegram-safe HTML.
    fn markdown_to_telegram_html(text: &str) -> String {
        if text.is_empty() {
            return String::new();
        }

        let patterns = MarkdownPatterns::get();

        // Escape HTML special characters first
        let mut result = text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");

        // Code blocks first (to prevent processing markdown inside code)
        result = patterns
            .code_block
            .replace_all(&result, "<pre>$1</pre>")
            .into_owned();

        // Inline code
        result = patterns
            .code_inline
            .replace_all(&result, "<code>$1</code>")
            .into_owned();

        // Bold: **text** or __text__ -> <b>text</b>
        result = patterns
            .bold_asterisk
            .replace_all(&result, "<b>$1</b>")
            .into_owned();
        result = patterns
            .bold_underscore
            .replace_all(&result, "<b>$1</b>")
            .into_owned();

        // Italic: *text* or _text_ -> <i>text</i>
        result = patterns
            .italic_asterisk
            .replace_all(&result, "<i>$1</i>")
            .into_owned();
        result = patterns
            .italic_underscore
            .replace_all(&result, "<i>$1</i>")
            .into_owned();

        // Strikethrough: ~~text~~ -> <s>text</s>
        result = patterns
            .strikethrough
            .replace_all(&result, "<s>$1</s>")
            .into_owned();

        // Links: [text](url) -> <a href="url">text</a>
        result = patterns
            .link
            .replace_all(&result, r#"<a href="$2">$1</a>"#)
            .into_owned();

        result
    }

    /// Split a long message into chunks.
    fn split_message(text: &str, max_len: usize) -> Vec<String> {
        if text.len() <= max_len {
            return vec![text.to_string()];
        }

        let mut chunks = Vec::new();
        let mut current = String::new();

        for line in text.lines() {
            if current.len() + line.len() + 1 > max_len {
                if !current.is_empty() {
                    chunks.push(current);
                    current = String::new();
                }
                // Handle very long lines
                if line.len() > max_len {
                    for chunk in line.as_bytes().chunks(max_len) {
                        chunks.push(String::from_utf8_lossy(chunk).to_string());
                    }
                    continue;
                }
            }
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);
        }

        if !current.is_empty() {
            chunks.push(current);
        }

        chunks
    }
}

/// Cached regex patterns for markdown to HTML conversion.
struct MarkdownPatterns {
    bold_asterisk: Regex,
    bold_underscore: Regex,
    italic_asterisk: Regex,
    italic_underscore: Regex,
    code_inline: Regex,
    code_block: Regex,
    strikethrough: Regex,
    link: Regex,
}

impl MarkdownPatterns {
    fn new() -> Self {
        Self {
            bold_asterisk: Regex::new(r"\*\*(.+?)\*\*").expect("valid regex"),
            bold_underscore: Regex::new(r"__(.+?)__").expect("valid regex"),
            // Simple italic patterns (process after bold to avoid conflicts)
            italic_asterisk: Regex::new(r"(?:^|[^*])\*([^*]+)\*(?:[^*]|$)").expect("valid regex"),
            italic_underscore: Regex::new(r"(?:^|[^_])_([^_]+)_(?:[^_]|$)").expect("valid regex"),
            code_inline: Regex::new(r"`([^`]+)`").expect("valid regex"),
            code_block: Regex::new(r"```\w*\n?([\s\S]*?)```").expect("valid regex"),
            strikethrough: Regex::new(r"~~(.+?)~~").expect("valid regex"),
            link: Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").expect("valid regex"),
        }
    }

    fn get() -> &'static Self {
        static PATTERNS: OnceLock<MarkdownPatterns> = OnceLock::new();
        PATTERNS.get_or_init(Self::new)
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    fn name(&self) -> &str {
        self.base.name()
    }

    async fn start(&self, bus: &MessageBus) -> ChannelResult<()> {
        self.base.set_state(ChannelState::Starting).await;

        let bot = Bot::new(&self.config.token);
        *self.bot.write().await = Some(bot.clone());

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        *self.shutdown_tx.write().await = Some(shutdown_tx);

        // Clone configuration for the handler
        let allowed_users = self.config.allowed_users.clone();
        let allowed_chats = self.config.allowed_chats.clone();
        let bus_handle = bus.inbound_handle();

        // Subscribe to outbound messages
        let mut outbound_rx = bus.subscribe_channel("telegram").await;
        let bot_for_output = bot.clone();
        let max_len = self.config.max_message_length;
        let parse_md = self.config.parse_markdown;

        // Spawn output handler
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(msg) = outbound_rx.recv() => {
                        let Ok(id) = msg.chat_id.parse::<i64>() else {
                            error!(chat_id = %msg.chat_id, "invalid chat ID");
                            continue;
                        };
                        let chat_id = ChatId(id);

                        let content = if parse_md && msg.format == MessageFormat::Markdown {
                            Self::markdown_to_telegram_html(&msg.content)
                        } else {
                            msg.content.clone()
                        };

                        // Split long messages
                        let chunks = Self::split_message(&content, max_len);

                        for chunk in chunks {
                            let result = if parse_md {
                                bot_for_output
                                    .send_message(chat_id, &chunk)
                                    .parse_mode(ParseMode::Html)
                                    .await
                            } else {
                                bot_for_output.send_message(chat_id, &chunk).await
                            };

                            if let Err(e) = result {
                                error!(error = %e, "failed to send telegram message");
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Telegram output handler shutting down");
                        break;
                    }
                }
            }
        });

        // Clone confirmation manager for callback handler
        let confirmation_manager = Arc::clone(&self.confirmation_manager);

        // Create message handler
        let message_handler = Update::filter_message().endpoint(move |_bot: Bot, msg: Message| {
            let bus_handle = bus_handle.clone();
            let allowed_users = allowed_users.clone();
            let allowed_chats = allowed_chats.clone();

            async move {
                // Check allowlists
                #[allow(clippy::cast_possible_wrap)] // User ID won't exceed i64 max
                let user_id = msg.from.as_ref().map_or(0, |u| u.id.0 as i64);
                let chat_id = msg.chat.id.0;

                let user_allowed = allowed_users.is_empty() || allowed_users.contains(&user_id);
                let chat_allowed = allowed_chats.is_empty() || allowed_chats.contains(&chat_id);

                if !user_allowed || !chat_allowed {
                    debug!(
                        user_id = user_id,
                        chat_id = chat_id,
                        "message from unauthorized user/chat"
                    );
                    return Ok::<(), teloxide::RequestError>(());
                }

                // Extract message content
                let content = match &msg.kind {
                    MessageKind::Common(common) => match &common.media_kind {
                        MediaKind::Text(text) => text.text.clone(),
                        _ => {
                            // Handle other media types
                            "[Media message]".to_string()
                        }
                    },
                    _ => return Ok(()),
                };

                // Create inbound message
                let inbound = InboundMessage::new(
                    "telegram",
                    user_id.to_string(),
                    chat_id.to_string(),
                    content,
                );

                // Publish to bus
                if let Err(e) = bus_handle.publish(inbound).await {
                    error!(error = %e, "failed to publish telegram message to bus");
                }

                Ok(())
            }
        });

        // Create callback query handler for confirmation buttons
        let callback_handler =
            Update::filter_callback_query().endpoint(move |bot: Bot, query: CallbackQuery| {
                let manager = Arc::clone(&confirmation_manager);

                async move {
                    let Some(data) = query.data else {
                        return Ok::<(), teloxide::RequestError>(());
                    };

                    // Parse callback data: "confirm:<request_id>:<action>"
                    if !data.starts_with("confirm:") {
                        return Ok(());
                    }

                    let parts: Vec<&str> = data.split(':').collect();
                    if parts.len() != 3 {
                        return Ok(());
                    }

                    let request_id = parts[1];
                    let action = parts[2];

                    // Determine response based on action
                    let response = match action {
                        "y" => crate::agent::confirmation::ConfirmationResponse::Approved,
                        "a" => crate::agent::confirmation::ConfirmationResponse::ApproveAll,
                        _ => crate::agent::confirmation::ConfirmationResponse::Denied,
                    };

                    // Send response to pending request
                    manager.respond(request_id, response).await;

                    // Answer the callback query to remove loading state
                    let answer_text = match action {
                        "y" => "✅ Approved",
                        "a" => "✅ Approved (all future calls)",
                        _ => "❌ Denied",
                    };

                    if let Err(e) = bot.answer_callback_query(query.id.clone()).text(answer_text).await {
                        error!(error = %e, "failed to answer callback query");
                    }

                    // Edit the original message to show the result
                    if let Some(msg) = query.message {
                        let new_text = format!(
                            "{}",
                            match action {
                                "y" => "✅ Tool execution approved",
                                "a" => "✅ Tool execution approved (all future calls)",
                                _ => "❌ Tool execution denied",
                            }
                        );
                        let _ = bot
                            .edit_message_text(msg.chat().id, msg.id(), new_text)
                            .await;
                    }

                    Ok(())
                }
            });

        // Combine handlers
        let handler = dptree::entry()
            .branch(message_handler)
            .branch(callback_handler);

        // Start the dispatcher
        let mut dispatcher = Dispatcher::builder(bot, handler)
            .enable_ctrlc_handler()
            .build();

        tokio::spawn(async move {
            dispatcher.dispatch().await;
        });

        self.base.set_state(ChannelState::Running).await;
        info!("Telegram channel started");

        Ok(())
    }

    async fn stop(&self) -> ChannelResult<()> {
        self.base.set_state(ChannelState::Stopping).await;

        // Send shutdown signal
        {
            let guard = self.shutdown_tx.write().await;
            if let Some(tx) = &*guard {
                let _ = tx.send(()).await;
            }
        }

        *self.bot.write().await = None;

        self.base.set_state(ChannelState::Stopped).await;
        info!("Telegram channel stopped");

        Ok(())
    }

    async fn send(&self, msg: &OutboundMessage) -> ChannelResult<()> {
        let bot = self.bot.read().await;
        let bot = bot.as_ref().ok_or(ChannelError::NotConnected)?;

        let chat_id: i64 = msg
            .chat_id
            .parse()
            .map_err(|_| ChannelError::SendFailed("invalid chat ID".to_string()))?;

        let content = if self.config.parse_markdown && msg.format == MessageFormat::Markdown {
            Self::markdown_to_telegram_html(&msg.content)
        } else {
            msg.content.clone()
        };

        let chunks = Self::split_message(&content, self.config.max_message_length);

        for chunk in chunks {
            let result = if self.config.parse_markdown {
                bot.send_message(ChatId(chat_id), &chunk)
                    .parse_mode(ParseMode::Html)
                    .await
            } else {
                bot.send_message(ChatId(chat_id), &chunk).await
            };

            result.map_err(|e| ChannelError::SendFailed(e.to_string()))?;
        }

        self.base.record_sent().await;
        Ok(())
    }

    async fn status(&self) -> ChannelStatus {
        self.base.build_status().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = TelegramChannelConfig::new("token123")
            .allow_user(12345)
            .allow_chat(67890)
            .parse_markdown(false);

        assert_eq!(config.token, "token123");
        assert!(config.is_user_allowed(12345));
        assert!(!config.is_user_allowed(99999));
        assert!(config.is_chat_allowed(67890));
        assert!(!config.parse_markdown);
    }

    #[test]
    fn test_split_message() {
        let short = "Hello, world!";
        let chunks = TelegramChannel::split_message(short, 100);
        assert_eq!(chunks.len(), 1);

        let long = "Line 1\nLine 2\nLine 3\nLine 4";
        let chunks = TelegramChannel::split_message(long, 15);
        assert!(chunks.len() > 1);
    }

    #[test]
    fn test_empty_allowlist() {
        let config = TelegramChannelConfig::new("token");
        // Empty allowlist means allow all
        assert!(config.is_user_allowed(12345));
        assert!(config.is_chat_allowed(67890));
    }

    #[test]
    fn test_markdown_to_html() {
        // Bold
        assert_eq!(
            TelegramChannel::markdown_to_telegram_html("**bold**"),
            "<b>bold</b>"
        );

        // Italic
        assert_eq!(
            TelegramChannel::markdown_to_telegram_html("*italic*"),
            "<i>italic</i>"
        );

        // Code
        assert_eq!(
            TelegramChannel::markdown_to_telegram_html("`code`"),
            "<code>code</code>"
        );

        // Links
        assert_eq!(
            TelegramChannel::markdown_to_telegram_html("[text](https://example.com)"),
            r#"<a href="https://example.com">text</a>"#
        );

        // HTML escaping
        assert_eq!(
            TelegramChannel::markdown_to_telegram_html("<script>"),
            "&lt;script&gt;"
        );
    }
}
