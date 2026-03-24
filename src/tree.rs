// File: src/tree.rs
use crate::walker::is_path_excluded;
use ignore::WalkBuilder;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct TreeGenerator {
    root: PathBuf,
    exclude_dir: PathBuf,
    output_file: PathBuf,
    allowed_paths: HashSet<PathBuf>,
    exclude_patterns: Vec<String>,
}

impl TreeGenerator {
    pub fn new(
        root: &Path,
        exclude_dir: &Path,
        output_file: &Path,
        exclude_patterns: &[String],
    ) -> Self {
        let canon = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        let root_canon = canon(root);

        let mut allowed_paths = HashSet::new();
        for entry in WalkBuilder::new(&root_canon)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .add_custom_ignore_filename(".barkignore")
            .build()
            .flatten()
        {
            let p = entry
                .path()
                .canonicalize()
                .unwrap_or_else(|_| entry.path().to_path_buf());
            allowed_paths.insert(p);
        }

        Self {
            root: root_canon,
            exclude_dir: canon(exclude_dir),
            output_file: canon(output_file),
            allowed_paths,
            exclude_patterns: exclude_patterns.to_vec(),
        }
    }

    pub fn generate(&self, output_path: &Path) -> anyhow::Result<String> {
        let mut out = String::from(".\n");
        self.walk(&self.root, "", &mut out)?;
        std::fs::write(output_path, &out)
            .map_err(|e| anyhow::anyhow!("writing tree.txt: {}", e))?;
        Ok(out)
    }

    fn walk(&self, dir: &Path, prefix: &str, out: &mut String) -> anyhow::Result<()> {
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

        // Respect .gitignore / .barkignore (and hidden files via WalkBuilder defaults)
        if !self.allowed_paths.contains(&canon) {
            return false;
        }

        // Respect config [exclude] patterns
        let rel = canon.strip_prefix(&self.root).unwrap_or(&canon);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        !is_path_excluded(&rel_str, &self.exclude_patterns)
    }
}
