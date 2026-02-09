use std::collections::HashMap;
use std::sync::Arc;

use tracing::{info, warn};

use crate::error::{KairoError, Result};
use crate::event_bus::EventBus;
use crate::plugin_trait::KairoPlugin;

pub struct PluginContext {
    pub services: HashMap<String, serde_json::Value>,
    pub event_bus: Arc<EventBus>,
}

pub struct PluginLoader {
    plugins: HashMap<String, Box<dyn KairoPlugin>>,
    order: Vec<String>,
    context: Arc<PluginContext>,
}

impl PluginLoader {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            plugins: HashMap::new(),
            order: Vec::new(),
            context: Arc::new(PluginContext {
                services: HashMap::new(),
                event_bus,
            }),
        }
    }

    pub fn register(&mut self, plugin: Box<dyn KairoPlugin>) {
        let meta = plugin.meta();
        info!("Registered plugin: {} ({})", meta.name, meta.id);
        self.plugins.insert(meta.id, plugin);
    }

    pub fn resolve_order(&mut self) -> Result<()> {
        let mut visited: HashMap<String, bool> = HashMap::new();
        let mut order: Vec<String> = Vec::new();

        let ids: Vec<String> = self.plugins.keys().cloned().collect();
        for id in &ids {
            if !visited.contains_key(id) {
                self.topo_visit(id, &mut visited, &mut order)?;
            }
        }

        self.order = order;
        info!("Plugin load order: {:?}", self.order);
        Ok(())
    }

    fn topo_visit(
        &self,
        id: &str,
        visited: &mut HashMap<String, bool>,
        order: &mut Vec<String>,
    ) -> Result<()> {
        if let Some(&in_progress) = visited.get(id) {
            if in_progress {
                return Err(KairoError::DependencyError(format!(
                    "Circular dependency detected at: {id}"
                )));
            }
            return Ok(());
        }

        visited.insert(id.to_string(), true);

        if let Some(plugin) = self.plugins.get(id) {
            let meta = plugin.meta();
            for dep in &meta.dependencies {
                if self.plugins.contains_key(&dep.plugin_id) {
                    self.topo_visit(&dep.plugin_id, visited, order)?;
                } else if !dep.optional {
                    return Err(KairoError::DependencyError(format!(
                        "Required dependency '{}' not found for plugin '{}'",
                        dep.plugin_id, id
                    )));
                } else {
                    warn!(
                        "Optional dependency '{}' not found for plugin '{}'",
                        dep.plugin_id, id
                    );
                }
            }
        }

        visited.insert(id.to_string(), false);
        order.push(id.to_string());
        Ok(())
    }

    pub async fn start_all(&mut self, config: &toml::Value) -> Result<()> {
        self.resolve_order()?;
        let order = self.order.clone();

        for id in &order {
            if let Some(plugin) = self.plugins.get_mut(id) {
                let plugin_config = config
                    .get("plugins")
                    .and_then(|p| p.get(id))
                    .cloned()
                    .unwrap_or(toml::Value::Table(toml::map::Map::new()));

                info!("Loading plugin: {id}");
                plugin.load(plugin_config).await?;
                info!("Initializing plugin: {id}");
                plugin.init().await?;
                info!("Starting plugin: {id}");
                plugin.start().await?;
            }
        }

        Ok(())
    }

    pub async fn stop_all(&mut self) -> Result<()> {
        let order: Vec<String> = self.order.iter().rev().cloned().collect();

        for id in &order {
            if let Some(plugin) = self.plugins.get_mut(id) {
                info!("Stopping plugin: {id}");
                if let Err(e) = plugin.stop().await {
                    warn!("Error stopping plugin {id}: {e}");
                }
            }
        }

        Ok(())
    }

    pub fn context(&self) -> &Arc<PluginContext> {
        &self.context
    }
}
