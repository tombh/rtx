use std::cmp::Ordering;
use color_eyre::eyre::Result;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use crate::config::Config;
use crate::plugins::{PluginName, Plugins};
use crate::toolset::{ToolVersion, ToolVersionOptions};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum ToolVersionRequest {
    Version(PluginName, String),
    Prefix(PluginName, String),
    Ref(PluginName, String),
    Path(PluginName, PathBuf),
    System(PluginName)
}

impl ToolVersionRequest {
    pub fn new(plugin_name: PluginName, s: &str) -> Self {
        match s.split_once(':') {
            Some(("ref", r)) => Self::Ref(plugin_name, r.to_string()),
            Some(("prefix", p)) => Self::Prefix(plugin_name, p.to_string()),
            Some(("path", p)) => Self::Path(plugin_name, PathBuf::from(p)),
            None => {
                if s == "system" {
                    Self::System(plugin_name)
                } else {
                    Self::Version(plugin_name, s.to_string())
                }
            }
            _ => panic!("invalid tool version request: {s}"),
        }
    }

    pub fn plugin_name(&self) -> &PluginName {
        match self {
            Self::Version(p, _) => p,
            Self::Prefix(p, _) => p,
            Self::Ref(p, _) => p,
            Self::Path(p, _) => p,
            Self::System(p) => p,
        }
    }

    pub fn version(&self) -> String {
        match self {
            Self::Version(_, v) => v.clone(),
            Self::Prefix(_, p) => format!("prefix-{p}"),
            Self::Ref(_, r) => format!("ref-{r}"),
            Self::Path(_, p) => format!("path-{}", p.display()),
            Self::System(_) => "system".to_string(),
        }
    }

    pub fn resolve(&self, config: &Config, plugin: &Plugins, opts: ToolVersionOptions, latest_versions: bool) -> Result<ToolVersion> {
        ToolVersion::resolve(config, plugin, self.clone(), opts, latest_versions)
    }
}

impl Display for ToolVersionRequest {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}@{}", self.plugin_name(), self.version())
    }
}

impl PartialOrd<Self> for ToolVersionRequest {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ToolVersionRequest {
    fn cmp(&self, other: &Self) -> Ordering {
        self.version().cmp(&other.version())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_str_eq;

    #[test]
    fn test_tool_version_request() {
        assert_str_eq!(ToolVersionRequest::new("1.2.3").to_string(), "1.2.3");
        assert_str_eq!(
            ToolVersionRequest::new("prefix:1.2.3").to_string(),
            "prefix:1.2.3"
        );
        assert_str_eq!(
            ToolVersionRequest::new("ref:1.2.3").to_string(),
            "ref:1.2.3"
        );
        assert_str_eq!(
            ToolVersionRequest::new("path:/foo/bar").to_string(),
            "path:/foo/bar"
        );
        assert_str_eq!(ToolVersionRequest::new("system").to_string(), "system");
    }
}
