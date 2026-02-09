use thiserror::Error;

#[derive(Debug, Error)]
pub enum KairoError {
    #[error("Plugin not found: {0}")]
    PluginNotFound(String),

    #[error("Plugin load error: {0}")]
    PluginLoadError(String),

    #[error("Plugin init error: {0}")]
    PluginInitError(String),

    #[error("Dependency resolution error: {0}")]
    DependencyError(String),

    #[error("Config error: {0}")]
    ConfigError(String),

    #[error("Session error: {0}")]
    SessionError(String),

    #[error("Inference error: {0}")]
    InferenceError(String),

    #[error("Event bus error: {0}")]
    EventBusError(String),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, KairoError>;
