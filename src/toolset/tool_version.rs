use std::fmt::{Display, Formatter};
use std::fs;
use std::path::PathBuf;

use color_eyre::eyre::Result;
use versions::{Chunk, Version};

use crate::config::Config;
use crate::dirs;
use crate::hash::hash_to_str;
use crate::plugins::{Plugin, PluginName, Plugins};
use crate::runtime_symlinks::is_runtime_symlink;
use crate::toolset::{ToolVersionOptions, ToolVersionRequest};

/// represents a single version of a tool for a particular plugin
#[derive(Debug, Clone)]
pub struct ToolVersion {
    pub request: ToolVersionRequest,
    pub plugin_name: PluginName,
    pub version: String,
    pub install_path: PathBuf,
    pub cache_path: PathBuf,
    pub download_path: PathBuf,
    pub opts: ToolVersionOptions,
}

impl ToolVersion {
    pub fn new(plugin: &Plugins, request: ToolVersionRequest, opts: ToolVersionOptions, version: String) -> Self {
        let pathname = match &request {
            ToolVersionRequest::Version(_, v) => v.to_string(),
            ToolVersionRequest::Prefix(_, p) => format!("prefix-{}", p.to_string()),
            ToolVersionRequest::Ref(_, r) => format!("ref-{}", r.to_string()),
            ToolVersionRequest::Path(_, p) => format!("path-{}", hash_to_str(&p)),
            ToolVersionRequest::System(_) => "system".to_string(),
        };
        ToolVersion {
            plugin_name: plugin.name().to_string(),
            install_path: dirs::INSTALLS.join(plugin.name()).join(&pathname),
            cache_path: dirs::CACHE.join(plugin.name()).join(&pathname),
            download_path: dirs::DOWNLOADS.join(plugin.name()).join(&pathname),
            version,
            request,
            opts,
        }
    }

    pub fn resolve(config: &Config, plugin: &Plugins, request: ToolVersionRequest, opts: ToolVersionOptions, latest_versions: bool) -> Result<Self> {
        let tv = match &request {
            ToolVersionRequest::Version(_, v) => Self::resolve_version(config, plugin, request, latest_versions, v, opts)?,
            ToolVersionRequest::Prefix(_, prefix) => Self::resolve_prefix(config, plugin, request, prefix, opts)?,
            _ => Self::new(plugin, request, opts, request.version()),
        };
        Ok(tv)
    }

    fn resolve_version(
        config: &Config,
        plugin: &Plugins,
        request: ToolVersionRequest,
        latest_versions: bool,
        v: &str,
        opts: ToolVersionOptions,
    ) -> Result<ToolVersion> {
        let v = config.resolve_alias(plugin.name(), &v)?;
        match v.split_once(':') {
            Some(("ref", r)) => {
                return Ok(Self::resolve_ref(plugin, r.to_string(), opts));
            }
            Some(("path", p)) => {
                return Self::resolve_path(plugin, PathBuf::from(p), opts);
            }
            Some(("prefix", p)) => {
                return Self::resolve_prefix(config, plugin, request, p, opts);
            }
            _ => (),
        }

        let build = |v| Ok(Self::new(plugin, request.clone(), opts.clone(), v));

        let existing_path = dirs::INSTALLS.join(plugin.name()).join(&v);
        if existing_path.exists() && !is_runtime_symlink(&existing_path) {
            // if the version is already installed, no need to fetch all the remote versions
            return build(v);
        }

        if v == "latest" {
            if !latest_versions {
                if let Some(v) = plugin.latest_installed_version()? {
                    return build(v);
                }
            }
            if let Some(v) = plugin.latest_version(&config.settings, None)? {
                return build(v);
            }
        }
        if !latest_versions {
            let matches = plugin.list_installed_versions_matching(&v)?;
            if matches.contains(&v) {
                return build(v);
            }
        }
        let matches = plugin.list_versions_matching(&config.settings, &v)?;
        if matches.contains(&v) {
            return build(v);
        }
        if v.contains("!-") {
            if let Some(tv) = Self::resolve_bang(config, plugin, request.clone(), &v, &opts)? {
                return Ok(tv);
            }
        }
        Self::resolve_prefix(config, plugin, request, &v, opts)
    }

    /// resolve a version like `12.0.0!-1` which becomes `11.0.0`, `12.1.0!-0.1` becomes `12.0.0`
    fn resolve_bang(
        config: &Config,
        plugin: &Plugins,
        request: ToolVersionRequest,
        v: &str,
        opts: &ToolVersionOptions,
    ) -> Result<Option<Self>> {
        let (wanted, minus) = v.split_once("!-").unwrap();
        let wanted = match wanted {
            "latest" => plugin.latest_version(&config.settings, None)?.unwrap(),
            _ => config.resolve_alias(plugin.name(), wanted)?,
        };
        let wanted = version_sub(&wanted, minus);
        let tv = match plugin.latest_version(&config.settings, Some(wanted))? {
            Some(v) => Some(Self::new(plugin, request, opts.clone(), v.to_string())),
            None => None,
        };
        Ok(tv)
    }

    fn resolve_prefix(
        config: &Config,
        plugin: &Plugins,
        request: ToolVersionRequest,
        prefix: &str,
        opts: ToolVersionOptions,
    ) -> Result<Self> {
        let matches = plugin.list_versions_matching(&config.settings, &prefix)?;
        let v = match matches.last() {
            Some(v) => v,
            None => prefix,
            // None => Err(VersionNotFound(plugin.name.clone(), prefix.to_string()))?,
        };
        Ok(Self::new(plugin, request, opts, v.to_string()))
    }

    fn resolve_ref(
        plugin: &Plugins,
        r: String,
        opts: ToolVersionOptions,
    ) -> Self {
        let request = ToolVersionRequest::Ref(plugin.name().clone(), r.clone());
        Self::new(plugin, request, opts, request.version())
    }

    fn resolve_path(
        plugin: &Plugins,
        path: PathBuf,
        opts: ToolVersionOptions,
    ) -> Result<ToolVersion> {
        let path = fs::canonicalize(path)?;
        let request = ToolVersionRequest::Path(plugin.name().clone(), path.clone());
        Ok(Self::new(plugin, request, opts, request.version()))
    }
}

impl Display for ToolVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}@{}", &self.plugin_name, &self.version)
    }
}

/// subtracts sub from orig and removes suffix
/// e.g. version_sub("18.2.3", "2") -> "16"
/// e.g. version_sub("18.2.3", "0.1") -> "18.1"
fn version_sub(orig: &str, sub: &str) -> String {
    let mut orig = Version::new(orig).unwrap();
    let sub = Version::new(sub).unwrap();
    while orig.chunks.0.len() > sub.chunks.0.len() {
        orig.chunks.0.pop();
    }
    for (i, orig_chunk) in orig.clone().chunks.0.iter().enumerate() {
        let m = sub.nth(i).unwrap();
        orig.chunks.0[i] = Chunk::Numeric(orig_chunk.single_digit().unwrap() - m);
    }
    orig.to_string()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_str_eq;
    use crate::config::Settings;
    use crate::plugins::{ExternalPlugin, PluginName};

    use super::*;

    #[test]
    fn test_version_sub() {
        assert_str_eq!(version_sub("18.2.3", "2"), "16");
        assert_str_eq!(version_sub("18.2.3", "0.1"), "18.1");
    }
}
