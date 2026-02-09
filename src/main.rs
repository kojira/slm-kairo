use anyhow::Result;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    tracing::info!("SLM-Kairo starting...");
    
    // TODO: Load config, init plugins, run event loop
    tracing::info!("SLM-Kairo initialized. Press Ctrl+C to stop.");
    
    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down...");
    
    Ok(())
}
