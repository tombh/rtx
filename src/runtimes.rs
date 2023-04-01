use std::collections::HashMap;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::fs::{remove_file, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use color_eyre::eyre::{eyre, Result};
use console::style;
use once_cell::sync::Lazy;

use crate::cache::CacheManager;
use crate::config::Config;
use crate::config::Settings;
use crate::env_diff::{EnvDiff, EnvDiffOperation};
use crate::file::{create_dir_all, display_path, remove_all_with_warning};
use crate::hash::hash_to_str;
use crate::lock_file::LockFile;
use crate::plugins::Script::{Download, ExecEnv, Install};
use crate::plugins::{Plugin, Plugins, Script, ScriptManager};
use crate::tera::{get_tera, BASE_CONTEXT};
use crate::toolset::{ToolVersion};
use crate::ui::progress_report::{ProgressReport, PROG_TEMPLATE};
use crate::{dirs, env, fake_asdf, file};

/// These represent individual plugin@version pairs of runtimes
/// installed to ~/.local/share/rtx/runtimes
#[derive(Debug, Clone)]
pub struct RuntimeVersion {
    pub version: String,
    pub plugin: Arc<Plugins>,
    exec_env_cache: CacheManager<HashMap<String, String>>,
}

impl RuntimeVersion {
    pub fn new(config: &Config, plugin: Arc<Plugins>, version: String, tv: ToolVersion) -> Self {
        let exec_env_cache =
            Self::exec_env_cache(config, &tv, &plugin, &install_path, &cache_path).unwrap();

        Self {
            exec_env_cache,
            version,
            plugin,
        }
    }

    fn exec_env_cache(
        config: &Config,
        tv: &ToolVersion,
        plugin: &Arc<Plugins>,
        install_path: &Path,
        cache_path: &Path,
    ) -> Result<CacheManager<HashMap<String, String>>> {
        match plugin.as_ref() {
            Plugins::External(plugin) => {
                let exec_env_filename = match &plugin.toml.exec_env.cache_key {
                    Some(key) => {
                        let key = render_cache_key(config, tv, key)?;
                        let filename = format!("{}.msgpack.z", key);
                        cache_path.join("exec_env").join(filename)
                    }
                    None => cache_path.join("exec_env.msgpack.z"),
                };
                let cm = CacheManager::new(exec_env_filename)
                    .with_fresh_file(dirs::ROOT.clone())
                    .with_fresh_file(plugin.plugin_path.clone())
                    .with_fresh_file(install_path.to_path_buf());
                Ok(cm)
            }
        }
    }

    pub fn exec_env(&self) -> Result<&HashMap<String, String>> {
        if self.version.as_str() == "system" {
            return Ok(&*EMPTY_HASH_MAP);
        }
        if !self.script_man.script_exists(&ExecEnv) || *env::__RTX_SCRIPT {
            // if the script does not exist or we're running from within a script already
            // the second is to prevent infinite loops
            return Ok(&*EMPTY_HASH_MAP);
        }
        self.exec_env_cache.get_or_try_init(|| {
            let script = self.script_man.get_script_path(&ExecEnv);
            let ed = EnvDiff::from_bash_script(&script, &self.script_man.env)?;
            let env = ed
                .to_patches()
                .into_iter()
                .filter_map(|p| match p {
                    EnvDiffOperation::Add(key, value) => Some((key, value)),
                    EnvDiffOperation::Change(key, value) => Some((key, value)),
                    _ => None,
                })
                .collect();
            Ok(env)
        })
    }
}
