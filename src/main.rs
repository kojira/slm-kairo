use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use kairo_core::load_config;
use kairo_plugin_inference::InferenceService;
use kairo_plugin_session::SessionService;
use kairo_plugin_gateway_discord::start_discord_bot;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!("SLM-Kairo starting...");

    let config_path = std::env::var("KAIRO_CONFIG")
        .unwrap_or_else(|_| "config/default.toml".to_string());
    let config = load_config(Path::new(&config_path))?;
    info!("Config loaded from {config_path}");

    let inference_config = config.get("plugins").and_then(|p| p.get("inference"));
    let api_url = inference_config.and_then(|c| c.get("api_url")).and_then(|v| v.as_str()).unwrap_or("http://localhost:8080/v1").to_string();
    let model = inference_config.and_then(|c| c.get("model")).and_then(|v| v.as_str()).unwrap_or("mlx-community/Qwen3-8B-4bit").to_string();
    let max_tokens = inference_config.and_then(|c| c.get("max_tokens")).and_then(|v| v.as_integer()).unwrap_or(2048) as u32;
    let temperature = inference_config.and_then(|c| c.get("temperature")).and_then(|v| v.as_float()).unwrap_or(0.7);
    let repetition_penalty = inference_config.and_then(|c| c.get("repetition_penalty")).and_then(|v| v.as_float());

    let session_config = config.get("plugins").and_then(|p| p.get("session"));
    let history_limit = session_config.and_then(|c| c.get("history_limit")).and_then(|v| v.as_integer()).unwrap_or(50) as usize;
    let system_prompt = session_config.and_then(|c| c.get("system_prompt")).and_then(|v| v.as_str()).unwrap_or("あなたはSLM-Kairoのテストエージェントです。").to_string();

    let discord_config = config.get("plugins").and_then(|p| p.get("gateway-discord"));
    let token_env = discord_config.and_then(|c| c.get("token_env")).and_then(|v| v.as_str()).unwrap_or("KAIRO_DISCORD_TOKEN");
    let token = std::env::var(token_env).unwrap_or_else(|_| panic!("Environment variable {token_env} must be set"));

    info!("Inference: api_url={api_url}, model={model}");
    info!("Session: history_limit={history_limit}");

    let inference = Arc::new(InferenceService::new(api_url, model, max_tokens, temperature, repetition_penalty));
    let session = Arc::new(SessionService::new(history_limit, system_prompt));

    let best_of_n = discord_config.and_then(|c| c.get("best_of_n")).and_then(|v| v.as_integer()).unwrap_or(4) as usize;

    info!("Starting Discord bot... (best_of_n={best_of_n})");
    start_discord_bot(token, inference, session, best_of_n).await?;

    Ok(())
}
