pub mod config;
pub mod error;
pub mod event_bus;
pub mod plugin_loader;
pub mod plugin_trait;
pub mod types;

pub use config::load_config;
pub use error::{KairoError, Result};
pub use event_bus::{EventBus, PluginEvent};
pub use plugin_loader::PluginLoader;
pub use plugin_trait::{
    HealthStatus, KairoPlugin, PluginCategory, PluginDependency, PluginHealth, PluginMeta,
};
pub use types::{ChatMessage, IncomingMessage, InferenceContext, Session};
