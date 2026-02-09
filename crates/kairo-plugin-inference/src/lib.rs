use std::collections::HashMap;

use async_trait::async_trait;
use kairo_core::{
    HealthStatus, KairoPlugin, PluginCategory, PluginHealth, PluginMeta, Result,
    InferenceContext,
};
use reqwest::Client;
use serde::{Serialize, Deserialize};

pub struct InferencePlugin {
    config: serde_json::Value,
}

impl InferencePlugin {
    pub fn create() -> Box<dyn KairoPlugin> {
        Box::new(Self {
            config: serde_json::Value::Null,
        })
    }
}

#[async_trait]
impl KairoPlugin for InferencePlugin {
    fn meta(&self) -> PluginMeta {
        PluginMeta {
            id: "inference".into(),
            name: "Inference Plugin".into(),
            version: "0.1.0".into(),
            dependencies: vec![],
            provides: vec!["inference".into()],
            category: PluginCategory::Inference,
        }
    }

    async fn load(&mut self, config: toml::Value) -> Result<()> {
        tracing::info!("InferencePlugin loaded");
        self.config = serde_json::to_value(config.to_string())?;
        Ok(())
    }

    async fn init(&mut self) -> Result<()> {
        tracing::info!("InferencePlugin initialized");
        Ok(())
    }

    async fn start(&mut self) -> Result<()> {
        tracing::info!("InferencePlugin started");
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        tracing::info!("InferencePlugin stopped");
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ApiMessage>,
    max_tokens: u32,
    temperature: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    repetition_penalty: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ApiMessage,
}

#[derive(Clone)]
pub struct InferenceService {
    client: Client,
    api_url: String,
    model: String,
    max_tokens: u32,
    temperature: f64,
    repetition_penalty: Option<f64>,
}

impl InferenceService {
    pub fn new(api_url: String, model: String, max_tokens: u32, temperature: f64, repetition_penalty: Option<f64>) -> Self {
        Self { client: Client::new(), api_url, model, max_tokens, temperature, repetition_penalty }
    }

    pub async fn chat(&self, messages: Vec<(String, String)>) -> std::result::Result<String, String> {
        let api_messages: Vec<ApiMessage> = messages.into_iter().map(|(role, content)| ApiMessage { role, content }).collect();
        let req = ChatCompletionRequest {
            model: self.model.clone(),
            messages: api_messages,
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            repetition_penalty: self.repetition_penalty,
        };
        let url = format!("{}/chat/completions", self.api_url);
        tracing::info!("Inference request: {} messages, rep_penalty={:?}", req.messages.len(), req.repetition_penalty);
        let resp = self.client.post(&url).json(&req).send().await.map_err(|e| format!("HTTP error: {e}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("API error {status}: {body}"));
        }
        let data: ChatCompletionResponse = resp.json().await.map_err(|e| format!("Parse error: {e}"))?;
        data.choices.first().map(|c| c.message.content.clone()).ok_or_else(|| "No choices".to_string())
    }

    pub async fn chat_best_of_n(
        &self,
        messages: Vec<(String, String)>,
        n: usize,
        evaluator: &(dyn Fn(&str) -> f64 + Send + Sync),
    ) -> std::result::Result<String, String> {
        let futures: Vec<_> = (0..n).map(|_| self.chat(messages.clone())).collect();
        let results = futures::future::join_all(futures).await;
        let candidates: Vec<String> = results.into_iter().filter_map(|r| r.ok()).collect();
        if candidates.is_empty() {
            return Err("All candidates failed".to_string());
        }
        let best = candidates.into_iter().max_by(|a, b| {
            evaluator(a).partial_cmp(&evaluator(b)).unwrap_or(std::cmp::Ordering::Equal)
        }).unwrap();
        Ok(best)
    }
}
