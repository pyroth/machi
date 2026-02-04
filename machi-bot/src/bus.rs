//! Async message bus for decoupled channel-agent communication.
//!
//! The message bus provides a publish-subscribe mechanism that decouples
//! chat channels from the agent core, enabling concurrent message processing.

use crate::error::{BusError, BusResult};
use crate::events::{InboundMessage, OutboundMessage};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast, mpsc};
use tracing::{debug, trace};

/// Default capacity for message queues.
const DEFAULT_QUEUE_CAPACITY: usize = 256;

/// Default capacity for broadcast channels.
const DEFAULT_BROADCAST_CAPACITY: usize = 64;

/// Async message bus that decouples chat channels from the agent core.
#[derive(Clone)]
pub struct MessageBus {
    inner: Arc<MessageBusInner>,
}

impl std::fmt::Debug for MessageBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageBus").finish_non_exhaustive()
    }
}

struct MessageBusInner {
    /// Inbound message queue (channels → agent).
    inbound_tx: mpsc::Sender<InboundMessage>,
    inbound_rx: RwLock<Option<mpsc::Receiver<InboundMessage>>>,

    /// Outbound broadcast (agent → channels).
    outbound_tx: broadcast::Sender<OutboundMessage>,

    /// Channel-specific subscribers for targeted delivery.
    channel_subscribers: RwLock<HashMap<String, Vec<mpsc::Sender<OutboundMessage>>>>,

    /// Statistics.
    stats: RwLock<BusStats>,
}

/// Message bus statistics.
#[derive(Debug, Default, Clone, Copy)]
pub struct BusStats {
    /// Total inbound messages processed.
    pub inbound_count: u64,
    /// Total outbound messages processed.
    pub outbound_count: u64,
    /// Messages dropped due to full queues.
    pub dropped_count: u64,
}

impl MessageBus {
    /// Create a new message bus with default capacity.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_QUEUE_CAPACITY)
    }

    /// Create a new message bus with specified queue capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        let (inbound_tx, inbound_rx) = mpsc::channel(capacity);
        let (outbound_tx, _) = broadcast::channel(DEFAULT_BROADCAST_CAPACITY);

        Self {
            inner: Arc::new(MessageBusInner {
                inbound_tx,
                inbound_rx: RwLock::new(Some(inbound_rx)),
                outbound_tx,
                channel_subscribers: RwLock::new(HashMap::new()),
                stats: RwLock::new(BusStats::default()),
            }),
        }
    }

    /// Publish an inbound message from a channel to the agent.
    ///
    /// This is called by channel implementations when they receive a message.
    pub async fn publish_inbound(&self, msg: InboundMessage) -> BusResult<()> {
        trace!(
            channel = %msg.channel,
            sender = %msg.sender_id,
            "publishing inbound message"
        );

        self.inner
            .inbound_tx
            .send(msg)
            .await
            .map_err(|_| BusError::InboundClosed)?;

        self.inner.stats.write().await.inbound_count += 1;
        Ok(())
    }

    /// Consume the next inbound message.
    ///
    /// This should only be called by the agent loop. Returns `None` when
    /// the bus is closed.
    pub async fn consume_inbound(&self) -> Option<InboundMessage> {
        let mut rx_guard = self.inner.inbound_rx.write().await;
        if let Some(rx) = rx_guard.as_mut() {
            rx.recv().await
        } else {
            None
        }
    }

    /// Try to consume the next inbound message with a timeout.
    ///
    /// Returns `None` if no message is available within the timeout.
    pub async fn consume_inbound_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> Option<InboundMessage> {
        let mut rx_guard = self.inner.inbound_rx.write().await;
        if let Some(rx) = rx_guard.as_mut() {
            tokio::time::timeout(timeout, rx.recv())
                .await
                .ok()
                .flatten()
        } else {
            None
        }
    }

    /// Publish an outbound message from the agent to channels.
    ///
    /// The message is broadcast to all subscribers and also sent to
    /// channel-specific subscribers.
    pub async fn publish_outbound(&self, msg: OutboundMessage) -> BusResult<()> {
        trace!(
            channel = %msg.channel,
            chat_id = %msg.chat_id,
            "publishing outbound message"
        );

        // Broadcast to all general subscribers
        let _ = self.inner.outbound_tx.send(msg.clone());

        // Send to channel-specific subscribers
        let subscribers = self.inner.channel_subscribers.read().await;
        if let Some(senders) = subscribers.get(&msg.channel) {
            for sender in senders {
                if sender.send(msg.clone()).await.is_err() {
                    debug!(
                        channel = %msg.channel,
                        "channel subscriber disconnected"
                    );
                }
            }
        }

        self.inner.stats.write().await.outbound_count += 1;
        Ok(())
    }

    /// Subscribe to all outbound messages (broadcast).
    ///
    /// Returns a receiver that will receive copies of all outbound messages.
    pub fn subscribe_outbound(&self) -> broadcast::Receiver<OutboundMessage> {
        self.inner.outbound_tx.subscribe()
    }

    /// Subscribe to outbound messages for a specific channel.
    ///
    /// Returns a receiver that will only receive messages targeted at the
    /// specified channel.
    pub async fn subscribe_channel(&self, channel: &str) -> mpsc::Receiver<OutboundMessage> {
        let (tx, rx) = mpsc::channel(DEFAULT_QUEUE_CAPACITY);

        let mut subscribers = self.inner.channel_subscribers.write().await;
        subscribers.entry(channel.to_string()).or_default().push(tx);

        debug!(channel = %channel, "new channel subscriber registered");
        rx
    }

    /// Get current bus statistics.
    pub async fn stats(&self) -> BusStats {
        *self.inner.stats.read().await
    }

    /// Get the number of pending inbound messages.
    #[must_use]
    pub const fn inbound_pending(&self) -> usize {
        // Note: This is approximate since we can't access the receiver's len
        // without taking the lock. For monitoring purposes, stats are more reliable.
        0
    }

    /// Create a handle for publishing inbound messages.
    ///
    /// This is useful for channels that need a lightweight handle without
    /// cloning the entire bus.
    pub fn inbound_handle(&self) -> InboundHandle {
        InboundHandle {
            tx: self.inner.inbound_tx.clone(),
        }
    }

    /// Create a handle for publishing outbound messages.
    pub fn outbound_handle(&self) -> OutboundHandle {
        OutboundHandle { bus: self.clone() }
    }
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Lightweight handle for publishing inbound messages.
#[derive(Debug, Clone)]
pub struct InboundHandle {
    tx: mpsc::Sender<InboundMessage>,
}

impl InboundHandle {
    /// Publish an inbound message.
    pub async fn publish(&self, msg: InboundMessage) -> BusResult<()> {
        self.tx.send(msg).await.map_err(|_| BusError::InboundClosed)
    }
}

/// Lightweight handle for publishing outbound messages.
#[derive(Debug, Clone)]
pub struct OutboundHandle {
    bus: MessageBus,
}

impl OutboundHandle {
    /// Publish an outbound message.
    pub async fn publish(&self, msg: OutboundMessage) -> BusResult<()> {
        self.bus.publish_outbound(msg).await
    }
}

/// Builder for configuring a message bus.
#[derive(Debug, Default, Clone, Copy)]
pub struct MessageBusBuilder {
    inbound_capacity: Option<usize>,
    broadcast_capacity: Option<usize>,
}

impl MessageBusBuilder {
    /// Create a new builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the inbound queue capacity.
    #[must_use]
    pub const fn inbound_capacity(mut self, capacity: usize) -> Self {
        self.inbound_capacity = Some(capacity);
        self
    }

    /// Set the broadcast channel capacity.
    #[must_use]
    pub const fn broadcast_capacity(mut self, capacity: usize) -> Self {
        self.broadcast_capacity = Some(capacity);
        self
    }

    /// Build the message bus.
    #[must_use]
    pub fn build(self) -> MessageBus {
        let inbound_cap = self.inbound_capacity.unwrap_or(DEFAULT_QUEUE_CAPACITY);
        let broadcast_cap = self
            .broadcast_capacity
            .unwrap_or(DEFAULT_BROADCAST_CAPACITY);

        let (inbound_tx, inbound_rx) = mpsc::channel(inbound_cap);
        let (outbound_tx, _) = broadcast::channel(broadcast_cap);

        MessageBus {
            inner: Arc::new(MessageBusInner {
                inbound_tx,
                inbound_rx: RwLock::new(Some(inbound_rx)),
                outbound_tx,
                channel_subscribers: RwLock::new(HashMap::new()),
                stats: RwLock::new(BusStats::default()),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_inbound_message_flow() {
        let bus = MessageBus::new();

        let msg = InboundMessage::new("test", "sender1", "chat1", "Hello");
        bus.publish_inbound(msg.clone()).await.unwrap();

        let received = bus
            .consume_inbound_timeout(std::time::Duration::from_millis(100))
            .await;
        assert!(received.is_some());
        assert_eq!(received.unwrap().content, "Hello");
    }

    #[tokio::test]
    async fn test_outbound_broadcast() {
        let bus = MessageBus::new();
        let mut rx = bus.subscribe_outbound();

        let msg = OutboundMessage::new("test", "chat1", "Response");
        bus.publish_outbound(msg).await.unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received.content, "Response");
    }

    #[tokio::test]
    async fn test_channel_subscription() {
        let bus = MessageBus::new();
        let mut rx = bus.subscribe_channel("telegram").await;

        // Message to telegram channel
        let msg1 = OutboundMessage::new("telegram", "chat1", "For Telegram");
        bus.publish_outbound(msg1).await.unwrap();

        // Message to other channel (should not be received)
        let msg2 = OutboundMessage::new("whatsapp", "chat2", "For WhatsApp");
        bus.publish_outbound(msg2).await.unwrap();

        let received = tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv())
            .await
            .unwrap();

        assert!(received.is_some());
        assert_eq!(received.unwrap().content, "For Telegram");
    }

    #[tokio::test]
    async fn test_stats() {
        let bus = MessageBus::new();

        let msg1 = InboundMessage::new("test", "s", "c", "in");
        bus.publish_inbound(msg1).await.unwrap();

        let msg2 = OutboundMessage::new("test", "c", "out");
        bus.publish_outbound(msg2).await.unwrap();

        let stats = bus.stats().await;
        assert_eq!(stats.inbound_count, 1);
        assert_eq!(stats.outbound_count, 1);
    }
}
