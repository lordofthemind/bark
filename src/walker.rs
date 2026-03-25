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
        Self {
            root,
            config,
            output_path,
            backup_dir,
        }
    }

    pub fn walk(&self) -> Vec<WalkEntry> {
        let output_canon = self
            .output_path
            .canonicalize()
            .unwrap_or_else(|_| self.output_path.clone());
        let backup_canon = self
            .backup_dir
            .canonicalize()
            .unwrap_or_else(|_| self.backup_dir.clone());

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

            // Get extension or handle extensionless files by filename
            let fname = path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();

            let (ext, style) = if let Some(e) = path.extension() {
                let ext = e.to_string_lossy().to_lowercase();
                // Skip user-configured extension skip list
                if self.config.extensions.skip.contains(&ext.to_string()) {
                    continue;
                }
                let style = match self.resolve_style(&ext) {
                    Some(s) => s,
                    None => continue,
                };
                (ext.to_string(), style)
            } else {
                // Extensionless file — check filename_skip
                if self.config.extensions.filename_skip.contains(&fname) {
                    continue;
                }
                let style = match self.resolve_filename_style(&fname) {
                    Some(s) => s,
                    None => continue,
                };
                (fname.clone(), style)
            };
            let _ = ext; // used for pattern matching above

            // Get relative path for header text
            let rel = path.strip_prefix(&self.root).unwrap_or(path).to_path_buf();
            let rel_str = rel.to_string_lossy().replace('\\', "/");

            // Check user exclude patterns
            if is_path_excluded(&rel_str, &self.config.exclude.patterns) {
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

            entries.push(WalkEntry {
                abs_path: abs,
                rel_path: rel,
                style,
            });
        }

        entries
    }

    fn resolve_filename_style(&self, name: &str) -> Option<CommentStyle> {
        // User-defined custom filenames take priority
        for custom in &self.config.extensions.filenames {
            if custom.name.to_lowercase() == name {
                return match custom.style.as_str() {
                    "slash" => Some(CommentStyle::Slash),
                    "hash" => Some(CommentStyle::Hash),
                    "css" => Some(CommentStyle::Css),
                    "html" => Some(CommentStyle::Html),
                    _ => None,
                };
            }
        }
        CommentStyle::filename_to_style(name)
    }

    fn resolve_style(&self, ext: &str) -> Option<CommentStyle> {
        // User-defined custom extensions take priority
        for custom in &self.config.extensions.custom {
            if custom.ext == ext {
                return match custom.style.as_str() {
                    "slash" => Some(CommentStyle::Slash),
                    "hash" => Some(CommentStyle::Hash),
                    "css" => Some(CommentStyle::Css),
                    "html" => Some(CommentStyle::Html),
                    _ => None,
                };
            }
        }
        CommentStyle::from_ext(ext)
    }
}

pub fn is_path_excluded(path: &str, patterns: &[String]) -> bool {
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
        assert!(is_path_excluded("dist/foo.js", &["dist/**".to_string()]));
        assert!(is_path_excluded(
            "node_modules/lodash/index.js",
            &["node_modules/**".to_string()]
        ));
    }

    #[test]
    fn is_excluded_wildcard_ext() {
        assert!(is_path_excluded("app.min.js", &["*.min.*".to_string()]));
        assert!(is_path_excluded(
            "vendor.bundle.js",
            &["*.bundle.*".to_string()]
        ));
    }

    #[test]
    fn is_excluded_no_match() {
        let patterns = vec!["target/**".to_string(), "dist/**".to_string()];
        assert!(!is_path_excluded("src/main.rs", &patterns));
        assert!(!is_path_excluded("README.md", &patterns));
    }

    #[test]
    fn is_excluded_empty_patterns() {
        assert!(!is_path_excluded("src/main.rs", &[]));
    }

    #[test]
    fn resolve_style_custom_overrides_builtin() {
        let mut config = Config::default();
        config.extensions.custom = vec![CustomExtension {
            ext: "rs".to_string(),
            style: "hash".to_string(),
        }];
        let config = std::sync::Arc::new(config);
        let walker = Walker::new(
            std::path::PathBuf::from("."),
            config,
            std::path::PathBuf::from("tree.txt"),
            std::path::PathBuf::from(".barks"),
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
            std::path::PathBuf::from(".barks"),
        );
        assert_eq!(walker.resolve_style("go"), Some(CommentStyle::Slash));
        assert_eq!(walker.resolve_style("py"), Some(CommentStyle::Hash));
        assert_eq!(walker.resolve_style("xyz"), None);
    }

    #[test]
    fn resolve_style_custom_css_and_html() {
        let mut config = Config::default();
        config.extensions.custom = vec![
            CustomExtension {
                ext: "mytml".to_string(),
                style: "html".to_string(),
            },
            CustomExtension {
                ext: "mystyle".to_string(),
                style: "css".to_string(),
            },
            CustomExtension {
                ext: "myunknown".to_string(),
                style: "invalid".to_string(),
            },
        ];
        let config = std::sync::Arc::new(config);
        let walker = Walker::new(
            std::path::PathBuf::from("."),
            config,
            std::path::PathBuf::from("tree.txt"),
            std::path::PathBuf::from(".barks"),
        );
        assert_eq!(walker.resolve_style("mytml"), Some(CommentStyle::Html));
        assert_eq!(walker.resolve_style("mystyle"), Some(CommentStyle::Css));
        assert_eq!(walker.resolve_style("myunknown"), None); // invalid style → None
    }

    #[test]
    fn is_excluded_subdir_filename_match() {
        // File in subdir: full path doesn't match *.min.* but filename does
        assert!(is_path_excluded(
            "subdir/app.min.js",
            &["*.min.*".to_string()]
        ));
        assert!(is_path_excluded(
            "deep/nested/vendor.bundle.js",
            &["*.bundle.*".to_string()]
        ));
    }

    #[test]
    fn walk_finds_rs_file_in_temp_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("hello.rs"), "fn main() {}").unwrap();
        let mut config = Config::default();
        config.exclude.patterns = vec![];
        let walker = Walker::new(
            tmp.path().to_path_buf(),
            std::sync::Arc::new(config),
            tmp.path().join("bark.txt"),
            tmp.path().join(".barks"),
        );
        let entries = walker.walk();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].rel_path, std::path::PathBuf::from("hello.rs"));
    }

    #[test]
    fn walk_skips_file_in_extension_skip_list() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("hello.rs"), "fn main() {}").unwrap();
        let mut config = Config::default();
        config.exclude.patterns = vec![];
        config.extensions.skip = vec!["rs".to_string()];
        let walker = Walker::new(
            tmp.path().to_path_buf(),
            std::sync::Arc::new(config),
            tmp.path().join("bark.txt"),
            tmp.path().join(".barks"),
        );
        assert!(walker.walk().is_empty());
    }

    #[test]
    fn walk_skips_large_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let file = tmp.path().join("big.rs");
        std::fs::write(&file, "x".repeat(2048)).unwrap();
        let mut config = Config::default();
        config.exclude.patterns = vec![];
        config.general.max_file_size = 512;
        let walker = Walker::new(
            tmp.path().to_path_buf(),
            std::sync::Arc::new(config),
            tmp.path().join("bark.txt"),
            tmp.path().join(".barks"),
        );
        assert!(walker.walk().is_empty());
    }

    #[test]
    fn walk_skips_excluded_subdir_pattern() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join("dist")).unwrap();
        std::fs::write(tmp.path().join("dist/bundle.rs"), "fn x() {}").unwrap();
        let mut config = Config::default();
        config.exclude.patterns = vec!["dist/**".to_string()];
        let walker = Walker::new(
            tmp.path().to_path_buf(),
            std::sync::Arc::new(config),
            tmp.path().join("bark.txt"),
            tmp.path().join(".barks"),
        );
        assert!(walker.walk().is_empty());
    }

    #[test]
    fn walk_skips_unknown_extension() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("file.xyz123"), "content").unwrap();
        let mut config = Config::default();
        config.exclude.patterns = vec![];
        let walker = Walker::new(
            tmp.path().to_path_buf(),
            std::sync::Arc::new(config),
            tmp.path().join("bark.txt"),
            tmp.path().join(".barks"),
        );
        assert!(walker.walk().is_empty());
    }

    #[test]
    fn walk_finds_extensionless_file_with_known_name() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Makefile is a known extensionless filename → Hash style
        std::fs::write(tmp.path().join("Makefile"), "all: build\n").unwrap();
        let mut config = Config::default();
        config.exclude.patterns = vec![];
        let walker = Walker::new(
            tmp.path().to_path_buf(),
            std::sync::Arc::new(config),
            tmp.path().join("bark.txt"),
            tmp.path().join(".barks"),
        );
        let entries = walker.walk();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].style,
            crate::header::CommentStyle::Hash,
            "Makefile should use Hash style"
        );
    }

    #[test]
    fn walk_skips_filename_in_filename_skip_list() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("Makefile"), "all: build").unwrap();
        let mut config = Config::default();
        config.exclude.patterns = vec![];
        config.extensions.filename_skip = vec!["makefile".to_string()];
        let walker = Walker::new(
            tmp.path().to_path_buf(),
            std::sync::Arc::new(config),
            tmp.path().join("bark.txt"),
            tmp.path().join(".barks"),
        );
        let entries = walker.walk();
        assert!(!entries
            .iter()
            .any(|e| e.rel_path.to_string_lossy().contains("Makefile")));
    }

    #[test]
    fn resolve_filename_style_custom_html_and_css() {
        use crate::config::CustomFilename;
        let mut config = Config::default();
        config.extensions.filenames = vec![
            CustomFilename {
                name: "Jenkinsfile".to_string(),
                style: "hash".to_string(),
            },
            CustomFilename {
                name: "Appfile".to_string(),
                style: "slash".to_string(),
            },
            CustomFilename {
                name: "Webfile".to_string(),
                style: "html".to_string(),
            },
            CustomFilename {
                name: "Cssfile".to_string(),
                style: "css".to_string(),
            },
            CustomFilename {
                name: "Weirdfile".to_string(),
                style: "invalid".to_string(),
            },
        ];
        let config = std::sync::Arc::new(config);
        let walker = Walker::new(
            std::path::PathBuf::from("."),
            config,
            std::path::PathBuf::from("tree.txt"),
            std::path::PathBuf::from(".barks"),
        );
        assert_eq!(
            walker.resolve_filename_style("jenkinsfile"),
            Some(CommentStyle::Hash)
        );
        assert_eq!(
            walker.resolve_filename_style("appfile"),
            Some(CommentStyle::Slash)
        );
        assert_eq!(
            walker.resolve_filename_style("webfile"),
            Some(CommentStyle::Html)
        );
        assert_eq!(
            walker.resolve_filename_style("cssfile"),
            Some(CommentStyle::Css)
        );
        assert_eq!(walker.resolve_filename_style("weirdfile"), None);
    }
}
