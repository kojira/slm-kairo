use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serenity::all::{Client, Context, EventHandler, GatewayIntents, Message, ReactionType, Ready};
use kairo_plugin_inference::InferenceService;
use kairo_plugin_session::SessionService;
use kairo_core::{
    HealthStatus, KairoPlugin, PluginCategory, PluginHealth, PluginMeta, Result,
    InferenceContext,
};

pub struct GatewayDiscordPlugin {
    config: serde_json::Value,
}

impl GatewayDiscordPlugin {
    pub fn create() -> Box<dyn KairoPlugin> {
        Box::new(Self {
            config: serde_json::Value::Null,
        })
    }
}

#[async_trait]
impl KairoPlugin for GatewayDiscordPlugin {
    fn meta(&self) -> PluginMeta {
        PluginMeta {
            id: "gateway-discord".into(),
            name: "Gateway Discord Plugin".into(),
            version: "0.1.0".into(),
            dependencies: vec![],
            provides: vec!["gateway".into()],
            category: PluginCategory::Gateway,
        }
    }

    async fn load(&mut self, config: toml::Value) -> Result<()> {
        tracing::info!("GatewayDiscordPlugin loaded");
        self.config = serde_json::to_value(config.to_string())?;
        Ok(())
    }

    async fn init(&mut self) -> Result<()> {
        tracing::info!("GatewayDiscordPlugin initialized");
        Ok(())
    }

    async fn start(&mut self) -> Result<()> {
        tracing::info!("GatewayDiscordPlugin started");
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        tracing::info!("GatewayDiscordPlugin stopped");
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

struct Handler {
    inference: Arc<InferenceService>,
    session: Arc<SessionService>,
    best_of_n: usize,
    allowed_channels: Vec<String>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        // 自分自身のメッセージは無視
        if msg.author.id.get() == ctx.cache.current_user().id.get() { return; }
        // チャンネルフィルター
        if !self.allowed_channels.is_empty() && !self.allowed_channels.contains(&msg.channel_id.to_string()) {
            return;
        }
        // Bot連続発言のループ防止: 直近5件中assistant が3件以上ならBotメッセージは無視
        if msg.author.bot {
            let channel_id = msg.channel_id.to_string();
            let history = self.session.get_messages(&channel_id);
            let recent: Vec<_> = history.iter().rev().take(5).collect();
            let assistant_count = recent.iter().filter(|(role, _)| role == "assistant").count();
            if assistant_count >= 3 {
                tracing::info!("Loop guard: {assistant_count}/5 recent messages are assistant, skipping bot message");
                return;
            }
        }
        let _typing = msg.channel_id.start_typing(&ctx.http);
        let channel_id = msg.channel_id.to_string();
        // 推論用に一時的にメッセージ追加（NO_REPLY時は巻き戻す）
        self.session.add_message(&channel_id, "user", &msg.content);
        let messages = self.session.get_messages(&channel_id);
        let result = if self.best_of_n > 1 {
            self.inference.chat_best_of_n(messages, self.best_of_n, &|response| {
                kairo_plugin_evaluator::evaluate_persona(response, "ほわり").total
            }).await
        } else {
            self.inference.chat(messages).await
        };
        match result {
            Ok(reply) => {
                let trimmed = reply.trim();
                if trimmed == "NO_REPLY" || trimmed.contains("NO_REPLY") {
                    tracing::info!("Skipping reply (NO_REPLY detected)");
                    // NO_REPLY時はuserメッセージも履歴から除去
                    self.session.pop_last_message(&channel_id);
                    // 代わりに☁️リアクションをつける
                    let _ = msg.react(&ctx.http, ReactionType::Unicode("☁️".to_string())).await;
                    return;
                }
                self.session.add_message(&channel_id, "assistant", &reply);
                if let Err(e) = msg.channel_id.say(&ctx.http, &reply).await {
                    tracing::error!("Failed to send message: {e}");
                }
            }
            Err(e) => {
                tracing::error!("Inference error: {e}");
                self.session.pop_last_message(&channel_id);
            }
        }
    }

    async fn ready(&self, _ctx: Context, ready: Ready) {
        tracing::info!("Discord bot connected as {}", ready.user.name);
    }
}

pub async fn start_discord_bot(
    token: String,
    inference: Arc<InferenceService>,
    session: Arc<SessionService>,
    best_of_n: usize,
    allowed_channels: Vec<String>,
) -> anyhow::Result<()> {
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
    let handler = Handler { inference, session, best_of_n, allowed_channels };
    let mut client = Client::builder(&token, intents)
        .event_handler(handler)
        .await?;
    client.start().await?;
    Ok(())
}
