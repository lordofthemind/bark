// File: src/config.rs
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub template: TemplateConfig,
    #[serde(default)]
    pub exclude: ExcludeConfig,
    #[serde(default)]
    pub extensions: ExtensionsConfig,
    #[serde(default)]
    pub watch: WatchConfig,  // used for .bark.toml deserialization
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            template: TemplateConfig::default(),
            exclude: ExcludeConfig::default(),
            extensions: ExtensionsConfig::default(),
            watch: WatchConfig::default(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct GeneralConfig {
    #[serde(default = "default_output")]
    pub output: String,
    #[serde(default = "default_backup_dir")]
    pub backup_dir: String,
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    #[serde(default = "default_true")]
    pub backup: bool,
}

fn default_output() -> String { "tree.txt".into() }
fn default_backup_dir() -> String { ".bark_backups".into() }
fn default_max_file_size() -> u64 { 1_048_576 }
fn default_true() -> bool { true }

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            output: default_output(),
            backup_dir: default_backup_dir(),
            max_file_size: default_max_file_size(),
            backup: true,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct TemplateConfig {
    #[serde(default = "default_template")]
    pub default: String,
    #[serde(default = "default_date_format")]
    pub date_format: String,
    #[serde(default)]
    pub variables: HashMap<String, String>,
    #[serde(default)]
    pub overrides: HashMap<String, String>,
}

fn default_template() -> String { "File: {{file}}".into() }
fn default_date_format() -> String { "%Y-%m-%d".into() }

impl Default for TemplateConfig {
    fn default() -> Self {
        Self {
            default: default_template(),
            date_format: default_date_format(),
            variables: HashMap::new(),
            overrides: HashMap::new(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ExcludeConfig {
    #[serde(default = "default_exclude_patterns")]
    pub patterns: Vec<String>,
}

fn default_exclude_patterns() -> Vec<String> {
    vec![
        "*.min.*".into(),
        "*.bundle.*".into(),
        "dist/**".into(),
        "build/**".into(),
        "node_modules/**".into(),
        "vendor/**".into(),
        "target/**".into(),
    ]
}

impl Default for ExcludeConfig {
    fn default() -> Self {
        Self { patterns: default_exclude_patterns() }
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ExtensionsConfig {
    #[serde(default)]
    pub custom: Vec<CustomExtension>,
    #[serde(default)]
    pub skip: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CustomExtension {
    pub ext: String,
    pub style: String,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct WatchConfig {
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,
    #[serde(default)]
    pub ignore: Vec<String>,
}

fn default_debounce_ms() -> u64 { 500 }

impl Default for WatchConfig {
    fn default() -> Self {
        Self { debounce_ms: 500, ignore: Vec::new() }
    }
}

impl Config {
    /// Walk upward from start_dir looking for .bark.toml, then check user config dir.
    pub fn find_and_load(start_dir: &Path) -> Result<Option<Self>> {
        let mut current = start_dir.to_path_buf();
        loop {
            let candidate = current.join(".bark.toml");
            if candidate.is_file() {
                return Self::from_file(&candidate).map(Some);
            }
            if !current.pop() {
                break;
            }
        }
        if let Some(home) = home_dir() {
            let user_cfg = home.join(".config").join("bark").join("config.toml");
            if user_cfg.is_file() {
                return Self::from_file(&user_cfg).map(Some);
            }
        }
        Ok(None)
    }

    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("reading config: {}", path.display()))?;
        toml::from_str(&content)
            .with_context(|| format!("parsing config: {}", path.display()))
    }
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok())
        .map(PathBuf::from)
}
