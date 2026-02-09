use std::collections::HashMap;

use async_trait::async_trait;
use kairo_core::{
    HealthStatus, KairoPlugin, PluginCategory, PluginHealth, PluginMeta, Result,
    InferenceContext,
};

pub struct EvaluatorPlugin {
    config: serde_json::Value,
}

impl EvaluatorPlugin {
    pub fn create() -> Box<dyn KairoPlugin> {
        Box::new(Self {
            config: serde_json::Value::Null,
        })
    }
}

#[async_trait]
impl KairoPlugin for EvaluatorPlugin {
    fn meta(&self) -> PluginMeta {
        PluginMeta {
            id: "evaluator".into(),
            name: "Evaluator Plugin".into(),
            version: "0.1.0".into(),
            dependencies: vec![],
            provides: vec!["evaluator".into()],
            category: PluginCategory::Evaluator,
        }
    }

    async fn load(&mut self, config: toml::Value) -> Result<()> {
        tracing::info!("EvaluatorPlugin loaded");
        self.config = serde_json::to_value(config.to_string())?;
        Ok(())
    }

    async fn init(&mut self) -> Result<()> {
        tracing::info!("EvaluatorPlugin initialized");
        Ok(())
    }

    async fn start(&mut self) -> Result<()> {
        tracing::info!("EvaluatorPlugin started");
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        tracing::info!("EvaluatorPlugin stopped");
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

pub struct PersonaScore {
    pub total: f64,
    pub name_ok: bool,
    pub tone_score: f64,
    pub length_ok: bool,
    pub no_tech: bool,
    pub no_repeat: bool,
}

pub fn evaluate_persona(response: &str, _persona_name: &str) -> PersonaScore {
    // name_ok: 「のすたろう」「すたろう」と自称してたらfalse
    let name_ok = !response.contains("のすたろう") && !response.contains("すたろう");

    // tone_score
    let tone_markers = ["ほわ〜", "えらいの", "なの", "だよぉ", "だもん", "☁️"];
    let tone_count = tone_markers.iter().filter(|m| response.contains(*m)).count();
    let tone_score = (tone_count as f64 / 6.0).min(1.0);

    // length_ok: 改行数が2以下
    let newline_count = response.chars().filter(|c| *c == '\n').count();
    let length_ok = newline_count <= 2;

    // no_tech
    let tech_terms = ["Rust", "cargo", "API", "build", "compile", "error", "debug", "config", "server", "deploy", "install", "git", "token", "session", "plugin", "inference", "HTTP", "JSON", "TOML"];
    let no_tech = !tech_terms.iter().any(|t| response.to_lowercase().contains(&t.to_lowercase()));

    // no_repeat: 同じ3文字以上のフレーズが5回以上繰り返されていない
    let no_repeat = {
        let chars: Vec<char> = response.chars().collect();
        let mut found_repeat = false;
        if chars.len() >= 3 {
            let mut freq: HashMap<String, usize> = HashMap::new();
            for i in 0..=chars.len().saturating_sub(3) {
                let phrase: String = chars[i..i+3].iter().collect();
                *freq.entry(phrase).or_insert(0) += 1;
            }
            found_repeat = freq.values().any(|&v| v >= 5);
        }
        !found_repeat
    };

    let total = (if name_ok { 0.3 } else { 0.0 })
        + tone_score * 0.2
        + (if length_ok { 0.2 } else { 0.0 })
        + (if no_tech { 0.15 } else { 0.0 })
        + (if no_repeat { 0.15 } else { 0.0 });

    PersonaScore { total, name_ok, tone_score, length_ok, no_tech, no_repeat }
}
