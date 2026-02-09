use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEvent {
    pub topic: String,
    pub payload: serde_json::Value,
    pub source_plugin: String,
}

#[derive(Debug)]
pub struct EventBus {
    sender: broadcast::Sender<PluginEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn broadcast(&self, event: PluginEvent) -> crate::error::Result<()> {
        self.sender.send(event).map_err(|e| {
            crate::error::KairoError::EventBusError(format!("Failed to broadcast event: {e}"))
        })?;
        Ok(())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<PluginEvent> {
        self.sender.subscribe()
    }
}
