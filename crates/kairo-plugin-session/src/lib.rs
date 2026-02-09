use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use kairo_core::{
    HealthStatus, KairoPlugin, PluginCategory, PluginHealth, PluginMeta, Result,
    InferenceContext,
};

pub struct SessionPlugin {
    config: serde_json::Value,
}

impl SessionPlugin {
    pub fn create() -> Box<dyn KairoPlugin> {
        Box::new(Self {
            config: serde_json::Value::Null,
        })
    }
}

#[async_trait]
impl KairoPlugin for SessionPlugin {
    fn meta(&self) -> PluginMeta {
        PluginMeta {
            id: "session".into(),
            name: "Session Plugin".into(),
            version: "0.1.0".into(),
            dependencies: vec![],
            provides: vec!["session".into()],
            category: PluginCategory::Session,
        }
    }

    async fn load(&mut self, config: toml::Value) -> Result<()> {
        tracing::info!("SessionPlugin loaded");
        self.config = serde_json::to_value(config.to_string())?;
        Ok(())
    }

    async fn init(&mut self) -> Result<()> {
        tracing::info!("SessionPlugin initialized");
        Ok(())
    }

    async fn start(&mut self) -> Result<()> {
        tracing::info!("SessionPlugin started");
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        tracing::info!("SessionPlugin stopped");
        Ok(())
    }

    async fn on_message(&self, _ctx: &mut InferenceContext) -> Result<()> {
        Ok(())
    }

    async fn pre_inference(&self, _ctx: &mut InferenceContext) -> Result<()> {
        Ok(())
    }

    async fn post_inference(&self, _ctx: &mut InferenceContext) -> Result<()> {
        Ok(())
    }

    fn current_config(&self) -> serde_json::Value {
        self.config.clone()
    }

    async fn update_config(&mut self, config: serde_json::Value) -> Result<()> {
        self.config = config;
        Ok(())
    }

    async fn health(&self) -> PluginHealth {
        PluginHealth {
            status: HealthStatus::Healthy,
            message: "OK".into(),
            metrics: HashMap::new(),
        }
    }
}

pub struct SessionService {
    sessions: Mutex<HashMap<String, Vec<(String, String)>>>,
    history_limit: usize,
    system_prompt: String,
}

impl SessionService {
    pub fn new(history_limit: usize, system_prompt: String) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            history_limit,
            system_prompt,
        }
    }

    pub fn add_message(&self, channel_id: &str, role: &str, content: &str) {
        let mut sessions = self.sessions.lock().unwrap();
        let history = sessions.entry(channel_id.to_string()).or_insert_with(Vec::new);
        history.push((role.to_string(), content.to_string()));
        if history.len() > self.history_limit {
            history.remove(0);
        }
    }

    pub fn pop_last_message(&self, channel_id: &str) {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(history) = sessions.get_mut(channel_id) {
            history.pop();
        }
    }

    pub fn get_messages(&self, channel_id: &str) -> Vec<(String, String)> {
        let sessions = self.sessions.lock().unwrap();
        let mut msgs = vec![("system".to_string(), self.system_prompt.clone())];
        if let Some(history) = sessions.get(channel_id) {
            msgs.extend(history.clone());
        }
        msgs
    }
}
