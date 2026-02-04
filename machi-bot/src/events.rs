//! Message events for channel communication.
//!
//! This module defines the core message types that flow through the message bus,
//! enabling decoupled communication between channels and the agent.

use crate::util::generate_message_id;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Media attachment in a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaAttachment {
    /// Media type (e.g., "image", "audio", "video", "document").
    pub media_type: MediaType,
    /// URL or file path to the media.
    pub url: String,
    /// Optional MIME type.
    pub mime_type: Option<String>,
    /// Optional file name.
    pub file_name: Option<String>,
    /// File size in bytes, if known.
    pub file_size: Option<u64>,
}

/// Type of media attachment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaType {
    /// Image file (jpg, png, gif, etc.)
    Image,
    /// Audio file or voice message
    Audio,
    /// Video file
    Video,
    /// Document or other file
    Document,
    /// Sticker
    Sticker,
}

/// An inbound message from a channel to the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    /// Unique message ID.
    pub id: String,
    /// Channel identifier (e.g., "telegram", "whatsapp", "cli").
    pub channel: String,
    /// Sender's identifier within the channel.
    pub sender_id: String,
    /// Chat/conversation identifier.
    pub chat_id: String,
    /// Message text content.
    pub content: String,
    /// Optional media attachments.
    #[serde(default)]
    pub media: Vec<MediaAttachment>,
    /// Timestamp when the message was received.
    pub timestamp: SystemTime,
    /// Optional reply-to message ID.
    pub reply_to: Option<String>,
    /// Additional metadata.
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl InboundMessage {
    /// Create a new inbound message with minimal required fields.
    pub fn new(
        channel: impl Into<String>,
        sender_id: impl Into<String>,
        chat_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            id: generate_message_id(),
            channel: channel.into(),
            sender_id: sender_id.into(),
            chat_id: chat_id.into(),
            content: content.into(),
            media: Vec::new(),
            timestamp: SystemTime::now(),
            reply_to: None,
            metadata: serde_json::Value::Null,
        }
    }

    /// Create a system message (internal communication).
    pub fn system(
        sender_id: impl Into<String>,
        chat_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self::new("system", sender_id, chat_id, content)
    }

    /// Create a CLI message.
    pub fn cli(content: impl Into<String>) -> Self {
        Self::new("cli", "user", "direct", content)
    }

    /// Get a unique session key for this conversation.
    #[must_use]
    pub fn session_key(&self) -> String {
        format!("{}:{}", self.channel, self.chat_id)
    }

    /// Add a media attachment.
    #[must_use]
    pub fn with_media(mut self, attachment: MediaAttachment) -> Self {
        self.media.push(attachment);
        self
    }

    /// Set reply-to message ID.
    #[must_use]
    pub fn with_reply_to(mut self, message_id: impl Into<String>) -> Self {
        self.reply_to = Some(message_id.into());
        self
    }

    /// Set metadata.
    #[must_use]
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

/// An outbound message from the agent to a channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    /// Unique message ID.
    pub id: String,
    /// Target channel identifier.
    pub channel: String,
    /// Target chat/conversation identifier.
    pub chat_id: String,
    /// Message text content.
    pub content: String,
    /// Optional media attachments.
    #[serde(default)]
    pub media: Vec<MediaAttachment>,
    /// Optional message ID to reply to.
    pub reply_to: Option<String>,
    /// Message format hint for the channel.
    pub format: MessageFormat,
    /// Additional metadata.
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Message format hint for rendering.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageFormat {
    /// Plain text, no formatting.
    #[default]
    Plain,
    /// Markdown formatted text.
    Markdown,
    /// HTML formatted text.
    Html,
}

impl OutboundMessage {
    /// Create a new outbound message.
    pub fn new(
        channel: impl Into<String>,
        chat_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            id: generate_message_id(),
            channel: channel.into(),
            chat_id: chat_id.into(),
            content: content.into(),
            media: Vec::new(),
            reply_to: None,
            format: MessageFormat::default(),
            metadata: serde_json::Value::Null,
        }
    }

    /// Create a response to an inbound message.
    pub fn reply_to(msg: &InboundMessage, content: impl Into<String>) -> Self {
        Self {
            id: generate_message_id(),
            channel: msg.channel.clone(),
            chat_id: msg.chat_id.clone(),
            content: content.into(),
            media: Vec::new(),
            reply_to: Some(msg.id.clone()),
            format: MessageFormat::Markdown,
            metadata: serde_json::Value::Null,
        }
    }

    /// Set message format.
    #[must_use]
    pub const fn with_format(mut self, format: MessageFormat) -> Self {
        self.format = format;
        self
    }

    /// Add a media attachment.
    #[must_use]
    pub fn with_media(mut self, attachment: MediaAttachment) -> Self {
        self.media.push(attachment);
        self
    }

    /// Set metadata.
    #[must_use]
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inbound_message_creation() {
        let msg = InboundMessage::new("telegram", "user123", "chat456", "Hello!");
        assert_eq!(msg.channel, "telegram");
        assert_eq!(msg.sender_id, "user123");
        assert_eq!(msg.chat_id, "chat456");
        assert_eq!(msg.content, "Hello!");
        assert_eq!(msg.session_key(), "telegram:chat456");
    }

    #[test]
    fn test_outbound_reply() {
        let inbound = InboundMessage::new("telegram", "user123", "chat456", "Hi");
        let outbound = OutboundMessage::reply_to(&inbound, "Hello back!");

        assert_eq!(outbound.channel, "telegram");
        assert_eq!(outbound.chat_id, "chat456");
        assert_eq!(outbound.reply_to, Some(inbound.id));
    }

    #[test]
    fn test_message_id_uniqueness() {
        let id1 = generate_message_id();
        let id2 = generate_message_id();
        assert_ne!(id1, id2);
    }
}
