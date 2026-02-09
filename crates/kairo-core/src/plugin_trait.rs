use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::types::InferenceContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMeta {
    pub id: String,
    pub name: String,
    pub version: String,
    pub dependencies: Vec<PluginDependency>,
    pub provides: Vec<String>,
    pub category: PluginCategory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDependency {
    pub plugin_id: String,
    pub version_req: String,
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PluginCategory {
    Core,
    Inference,
    Session,
    Memory,
    Gateway,
    Tools,
    Evaluator,
    Story,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHealth {
    pub status: HealthStatus,
    pub message: String,
    pub metrics: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[async_trait]
pub trait KairoPlugin: Send + Sync {
    fn meta(&self) -> PluginMeta;

    async fn load(&mut self, config: toml::Value) -> Result<()>;
    async fn init(&mut self) -> Result<()>;
    async fn start(&mut self) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;

    async fn on_message(&self, ctx: &mut InferenceContext) -> Result<()>;
    async fn pre_inference(&self, ctx: &mut InferenceContext) -> Result<()>;
    async fn post_inference(&self, ctx: &mut InferenceContext) -> Result<()>;

    fn current_config(&self) -> serde_json::Value;
    async fn update_config(&mut self, config: serde_json::Value) -> Result<()>;

    async fn health(&self) -> PluginHealth;
}
