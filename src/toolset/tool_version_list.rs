use crate::config::Config;
use crate::plugins::{Plugin, };
use crate::toolset::tool_version_request::ToolVersionRequest;
use crate::toolset::{ToolSource, ToolVersion, ToolVersionOptions};

/// represents several versions of a tool for a particular plugin
#[derive(Debug, Clone)]
pub struct ToolVersionList {
    pub plugin_name: String,
    pub versions: Vec<ToolVersion>,
    pub requests: Vec<(ToolVersionRequest, ToolVersionOptions)>,
    pub source: ToolSource,
}

impl ToolVersionList {
    pub fn new(plugin_name: String, source: ToolSource) -> Self {
        Self {
            plugin_name,
            versions: Vec::new(),
            requests: vec![],
            source,
        }
    }
    pub fn resolve(&mut self, config: &Config, latest_versions: bool) {
        let plugin = match config.plugins.get(&self.plugin_name) {
            Some(p) if p.is_installed() => p,
            _ => {
                debug!("Plugin {} is not installed", self.plugin_name);
                return;
            }
        };
        for (tvr, opts) in &mut self.requests {
            match tvr.resolve(config, plugin, opts.clone(), latest_versions) {
                Ok(v) => self.versions.push(v),
                Err(err) => warn!("failed to resolve tool version: {:#}", err),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::sync::Arc;

    use crate::config::Config;
    use crate::plugins::{ExternalPlugin, Plugin, PluginName, Plugins};
    use crate::toolset::{ToolSource, ToolVersion, ToolVersionList, ToolVersionType};

    #[test]
    fn test_tool_version_list_failure() {
        env::set_var("RTX_FAILURE", "1");
        let plugin_name = String::from("dummy");
        let mut config = Config::default();
        config.plugins.insert(
            plugin_name.clone(),
            Arc::new(Plugins::External(ExternalPlugin::new(
                &config.settings,
                &PluginName::from("dummy"),
            ))),
        );
        let mut tvl = ToolVersionList::new(plugin_name, ToolSource::Argument);
        let settings = crate::config::Settings::default();
        let plugin = Arc::new(Plugins::External(ExternalPlugin::new(
            &settings,
            &PluginName::from("dummy"),
        )));
        plugin.clear_remote_version_cache().unwrap();
        tvl.add_version(ToolVersion::new(
            plugin.name().to_string(),
            ToolVersionType::Version("1.0.0".to_string()),
        ));
        tvl.resolve(&config, false);
        assert_eq!(tvl.versions.len(), 0);
        env::remove_var("RTX_FAILURE");
    }
}
