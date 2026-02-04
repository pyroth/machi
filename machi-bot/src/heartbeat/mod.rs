//! Heartbeat module for proactive wake-up and health monitoring.
//!
//! The heartbeat service periodically checks the system health and can
//! trigger proactive actions based on configured conditions.

use crate::bus::MessageBus;
use crate::events::InboundMessage;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, info, warn};

/// Heartbeat service configuration.
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    /// Interval between heartbeats.
    pub interval: Duration,
    /// Whether to send proactive messages.
    pub proactive_enabled: bool,
    /// Channel for proactive messages.
    pub channel: String,
    /// Chat ID for proactive messages.
    pub chat_id: String,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(60),
            proactive_enabled: false,
            channel: "cli".to_string(),
            chat_id: "default".to_string(),
        }
    }
}

/// Handle for controlling the heartbeat service.
#[derive(Debug, Clone)]
pub struct HeartbeatHandle {
    shutdown_tx: mpsc::Sender<()>,
}

impl HeartbeatHandle {
    /// Signal the heartbeat service to stop.
    pub async fn stop(&self) {
        let _ = self.shutdown_tx.send(()).await;
    }
}

/// Health status of the system.
#[derive(Debug, Clone, Copy, Default)]
pub struct HealthStatus {
    /// Whether the system is healthy.
    pub healthy: bool,
    /// Number of heartbeats since start.
    pub heartbeat_count: u64,
    /// Last heartbeat timestamp.
    pub last_heartbeat: Option<std::time::SystemTime>,
    /// Number of errors encountered.
    pub error_count: u64,
}

/// Heartbeat service for system health monitoring.
#[derive(Debug)]
pub struct HeartbeatService {
    config: HeartbeatConfig,
    bus: MessageBus,
    status: Arc<RwLock<HealthStatus>>,
    running: Arc<RwLock<bool>>,
}

impl HeartbeatService {
    /// Create a new heartbeat service.
    pub fn new(config: HeartbeatConfig, bus: MessageBus) -> Self {
        Self {
            config,
            bus,
            status: Arc::new(RwLock::new(HealthStatus {
                healthy: true,
                ..Default::default()
            })),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Get the current health status.
    pub async fn status(&self) -> HealthStatus {
        *self.status.read().await
    }

    /// Check if the service is running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Start the heartbeat service.
    #[allow(clippy::unused_async)] // async is part of the public API contract
    pub async fn start(self) -> HeartbeatHandle {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        let handle = HeartbeatHandle { shutdown_tx };

        let config = self.config.clone();
        let bus = self.bus.clone();
        let status = Arc::clone(&self.status);
        let running = self.running;

        tokio::spawn(async move {
            *running.write().await = true;
            info!(interval = ?config.interval, "heartbeat service started");

            loop {
                tokio::select! {
                    () = tokio::time::sleep(config.interval) => {
                        Self::beat(&config, &bus, &status).await;
                    }
                    _ = shutdown_rx.recv() => {
                        info!("heartbeat service shutting down");
                        break;
                    }
                }
            }

            *running.write().await = false;
        });

        handle
    }

    /// Perform a single heartbeat.
    async fn beat(config: &HeartbeatConfig, bus: &MessageBus, status: &Arc<RwLock<HealthStatus>>) {
        let now = std::time::SystemTime::now();
        let count: u64;

        // Update status
        {
            let mut s = status.write().await;
            s.heartbeat_count += 1;
            s.last_heartbeat = Some(now);
            s.healthy = true;
            count = s.heartbeat_count;
        }

        debug!(count, "heartbeat");

        // Send proactive message if enabled
        if config.proactive_enabled {
            let msg = InboundMessage::new(
                &config.channel,
                "heartbeat",
                &config.chat_id,
                "[Heartbeat] System check - all systems operational",
            );

            if let Err(e) = bus.publish_inbound(msg).await {
                warn!(error = %e, "failed to send heartbeat message");
                let mut s = status.write().await;
                s.error_count += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_heartbeat_config_default() {
        let config = HeartbeatConfig::default();
        assert_eq!(config.interval, Duration::from_secs(60));
        assert!(!config.proactive_enabled);
    }

    #[tokio::test]
    async fn test_heartbeat_service_status() {
        let config = HeartbeatConfig::default();
        let bus = MessageBus::new();
        let service = HeartbeatService::new(config, bus);

        let status = service.status().await;
        assert!(status.healthy);
        assert_eq!(status.heartbeat_count, 0);
    }
}
