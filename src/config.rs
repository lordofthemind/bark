// File: src/config.rs
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
#[derive(Default)]
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
    pub watch: WatchConfig, // used for .bark.toml deserialization
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

fn default_output() -> String {
    "bark.txt".into()
}
fn default_backup_dir() -> String {
    ".barks".into()
}
fn default_max_file_size() -> u64 {
    1_048_576
}
fn default_true() -> bool {
    true
}

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

fn default_template() -> String {
    "File: {{file}}".into()
}
fn default_date_format() -> String {
    "%Y-%m-%d".into()
}

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
    #[serde(default)]
    pub header_skip: Vec<String>,
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
        Self {
            patterns: default_exclude_patterns(),
            header_skip: Vec::new(),
        }
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

fn default_debounce_ms() -> u64 {
    500
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            debounce_ms: 500,
            ignore: Vec::new(),
        }
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
        toml::from_str(&content).with_context(|| format!("parsing config: {}", path.display()))
    }
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults() {
        let c = Config::default();
        assert_eq!(c.general.output, "bark.txt");
        assert_eq!(c.general.backup_dir, ".barks");
        assert_eq!(c.general.max_file_size, 1_048_576);
        assert!(c.general.backup);
        assert_eq!(c.template.default, "File: {{file}}");
        assert_eq!(c.template.date_format, "%Y-%m-%d");
        assert!(!c.exclude.patterns.is_empty());
    }

    #[test]
    fn config_from_minimal_toml() {
        let toml = r#"
[general]
output = "my_tree.txt"
"#;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), toml).unwrap();
        let c = Config::from_file(tmp.path()).unwrap();
        assert_eq!(c.general.output, "my_tree.txt");
        // Everything else falls back to defaults
        assert_eq!(c.general.backup_dir, ".barks");
        assert_eq!(c.template.default, "File: {{file}}");
    }

    #[test]
    fn config_template_override() {
        let toml = r#"
[template]
default = "File: {{file}} | Author: {{author}}"
"#;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), toml).unwrap();
        let c = Config::from_file(tmp.path()).unwrap();
        assert_eq!(c.template.default, "File: {{file}} | Author: {{author}}");
    }

    #[test]
    fn config_custom_variables() {
        let toml = r#"
[template.variables]
author = "Bob"
team = "backend"
"#;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), toml).unwrap();
        let c = Config::from_file(tmp.path()).unwrap();
        assert_eq!(
            c.template.variables.get("author").map(|s| s.as_str()),
            Some("Bob")
        );
        assert_eq!(
            c.template.variables.get("team").map(|s| s.as_str()),
            Some("backend")
        );
    }

    #[test]
    fn config_invalid_toml_errors() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "not valid toml :::").unwrap();
        assert!(Config::from_file(tmp.path()).is_err());
    }

    #[test]
    fn config_find_and_load_no_config_file() {
        // A temp dir with no .bark.toml — find_and_load should walk up, check home, return None
        // (assumes no .bark.toml in /tmp ancestry and no ~/.config/bark/config.toml)
        let dir = tempfile::TempDir::new().unwrap();
        // Just verify it doesn't panic; result might be None or Some if user config exists
        let result = Config::find_and_load(dir.path());
        assert!(result.is_ok(), "find_and_load should not error");
    }

    #[test]
    fn home_dir_returns_some_on_unix() {
        // On Linux/macOS, HOME should be set
        if std::env::var("HOME").is_ok() {
            assert!(super::home_dir().is_some());
        }
    }

    #[test]
    fn watch_config_defaults() {
        let c = WatchConfig::default();
        assert_eq!(c.debounce_ms, 500);
        assert!(c.ignore.is_empty());
    }

    #[test]
    fn exclude_config_defaults() {
        let c = ExcludeConfig::default();
        assert!(c.patterns.contains(&"*.min.*".to_string()));
        assert!(c.patterns.contains(&"target/**".to_string()));
    }
}
