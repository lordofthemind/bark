// File: src/tree.rs
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct TreeGenerator {
    root: PathBuf,
    exclude_dir: PathBuf,
    output_file: PathBuf,
}

impl TreeGenerator {
    pub fn new(root: &Path, exclude_dir: &Path, output_file: &Path) -> Self {
        // Canonicalize so comparisons work regardless of how paths were specified
        let canon = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        Self {
            root: canon(root),
            exclude_dir: canon(exclude_dir),
            output_file: canon(output_file),
        }
    }

    pub fn generate(&self, output_path: &Path) -> Result<String> {
        let mut out = String::from(".\n");
        self.walk(&self.root, "", &mut out)?;
        std::fs::write(output_path, &out)
            .map_err(|e| anyhow::anyhow!("writing tree.txt: {}", e))?;
        Ok(out)
    }

    fn walk(&self, dir: &Path, prefix: &str, out: &mut String) -> Result<()> {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| self.should_include(p))
            .collect();

        entries.sort();

        for (i, entry) in entries.iter().enumerate() {
            let is_last = i == entries.len() - 1;
            let connector = if is_last { "└── " } else { "├── " };
            let name = entry
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            if entry.is_dir() {
                out.push_str(&format!("{}{}{}/\n", prefix, connector, name));
                let child_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
                self.walk(entry, &child_prefix, out)?;
            } else {
                out.push_str(&format!("{}{}{}\n", prefix, connector, name));
            }
        }
        Ok(())
    }

    fn should_include(&self, path: &Path) -> bool {
        let canon = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        // Skip backup dir and output file
        if canon == self.exclude_dir || canon == self.output_file {
            return false;
        }

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // Skip .git and other hidden dirs that clutter the tree
        if name.starts_with('.') {
            return false;
        }

        // Skip common build/dependency dirs
        matches!(
            name.as_str(),
            s if !["node_modules", "target", "dist", "build", "vendor", "__pycache__"].contains(&s)
        )
    }
}
