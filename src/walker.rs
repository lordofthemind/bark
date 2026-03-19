// File: src/walker.rs
use crate::config::Config;
use crate::header::CommentStyle;
use ignore::WalkBuilder;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct WalkEntry {
    pub abs_path: PathBuf,
    pub rel_path: PathBuf,
    pub style: CommentStyle,
}

pub struct Walker {
    root: PathBuf,
    config: Arc<Config>,
    output_path: PathBuf,
    backup_dir: PathBuf,
}

impl Walker {
    pub fn new(
        root: PathBuf,
        config: Arc<Config>,
        output_path: PathBuf,
        backup_dir: PathBuf,
    ) -> Self {
        Self { root, config, output_path, backup_dir }
    }

    pub fn walk(&self) -> Vec<WalkEntry> {
        let output_canon = self.output_path.canonicalize().unwrap_or_else(|_| self.output_path.clone());
        let backup_canon = self.backup_dir.canonicalize().unwrap_or_else(|_| self.backup_dir.clone());

        let walker = WalkBuilder::new(&self.root)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .add_custom_ignore_filename(".barkignore")
            .build();

        let mut entries = Vec::new();

        for result in walker {
            let dir_entry = match result {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = dir_entry.path();

            // Only process files
            if !path.is_file() {
                continue;
            }

            let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

            // Skip our own output / backup files
            if abs == output_canon || abs.starts_with(&backup_canon) {
                continue;
            }

            // Get extension
            let ext = match path.extension() {
                Some(e) => e.to_string_lossy().to_lowercase(),
                None => continue,
            };

            // Skip user-configured extension skip list
            if self.config.extensions.skip.contains(&ext.to_string()) {
                continue;
            }

            // Resolve comment style (custom extensions override built-ins)
            let style = match self.resolve_style(&ext) {
                Some(s) => s,
                None => continue,
            };

            // Get relative path for header text
            let rel = path.strip_prefix(&self.root)
                .unwrap_or(path)
                .to_path_buf();
            let rel_str = rel.to_string_lossy().replace('\\', "/");

            // Check user exclude patterns
            if is_excluded(&rel_str, &self.config.exclude.patterns) {
                continue;
            }

            // Skip files that are too large
            if let Ok(meta) = path.metadata() {
                if meta.len() > self.config.general.max_file_size {
                    continue;
                }
            }

            // Skip binary files
            if is_binary(path) {
                continue;
            }

            entries.push(WalkEntry { abs_path: abs, rel_path: rel, style });
        }

        entries
    }

    fn resolve_style(&self, ext: &str) -> Option<CommentStyle> {
        // User-defined custom extensions take priority
        for custom in &self.config.extensions.custom {
            if custom.ext == ext {
                return match custom.style.as_str() {
                    "slash" => Some(CommentStyle::Slash),
                    "hash"  => Some(CommentStyle::Hash),
                    "css"   => Some(CommentStyle::Css),
                    "html"  => Some(CommentStyle::Html),
                    _ => None,
                };
            }
        }
        CommentStyle::from_ext(ext)
    }
}

fn is_excluded(path: &str, patterns: &[String]) -> bool {
    for pattern in patterns {
        if let Ok(p) = glob::Pattern::new(pattern) {
            if p.matches(path) {
                return true;
            }
        }
        // Also try matching just the filename component against patterns like *.min.*
        if let Some(fname) = Path::new(path).file_name() {
            let fname_str = fname.to_string_lossy();
            if let Ok(p) = glob::Pattern::new(pattern) {
                if p.matches(&fname_str) {
                    return true;
                }
            }
        }
    }
    false
}

fn is_binary(path: &Path) -> bool {
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut buf = [0u8; 8192];
    let n = match file.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return false,
    };
    content_inspector::inspect(&buf[..n]) == content_inspector::ContentType::BINARY
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, CustomExtension};

    #[test]
    fn is_excluded_dir_glob() {
        assert!(is_excluded("dist/foo.js", &["dist/**".to_string()]));
        assert!(is_excluded("node_modules/lodash/index.js", &["node_modules/**".to_string()]));
    }

    #[test]
    fn is_excluded_wildcard_ext() {
        assert!(is_excluded("app.min.js", &["*.min.*".to_string()]));
        assert!(is_excluded("vendor.bundle.js", &["*.bundle.*".to_string()]));
    }

    #[test]
    fn is_excluded_no_match() {
        let patterns = vec!["target/**".to_string(), "dist/**".to_string()];
        assert!(!is_excluded("src/main.rs", &patterns));
        assert!(!is_excluded("README.md", &patterns));
    }

    #[test]
    fn is_excluded_empty_patterns() {
        assert!(!is_excluded("src/main.rs", &[]));
    }

    #[test]
    fn resolve_style_custom_overrides_builtin() {
        let mut config = Config::default();
        config.extensions.custom = vec![
            CustomExtension { ext: "rs".to_string(), style: "hash".to_string() },
        ];
        let config = std::sync::Arc::new(config);
        let walker = Walker::new(
            std::path::PathBuf::from("."),
            config,
            std::path::PathBuf::from("tree.txt"),
            std::path::PathBuf::from(".bark_backups"),
        );
        // .rs normally maps to Slash; custom override sets it to Hash
        assert_eq!(walker.resolve_style("rs"), Some(CommentStyle::Hash));
    }

    #[test]
    fn resolve_style_builtin_fallthrough() {
        let config = std::sync::Arc::new(Config::default());
        let walker = Walker::new(
            std::path::PathBuf::from("."),
            config,
            std::path::PathBuf::from("tree.txt"),
            std::path::PathBuf::from(".bark_backups"),
        );
        assert_eq!(walker.resolve_style("go"), Some(CommentStyle::Slash));
        assert_eq!(walker.resolve_style("py"), Some(CommentStyle::Hash));
        assert_eq!(walker.resolve_style("xyz"), None);
    }
}
