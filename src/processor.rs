// File: src/processor.rs
use crate::backup::BackupManager;
use crate::config::Config;
use crate::header::{self, CommentStyle, HeaderAction};
use crate::template::TemplateContext;
use crate::walker::{WalkEntry, Walker};
use colored::Colorize;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Default)]
pub struct Stats {
    pub tagged: AtomicUsize,
    pub updated: AtomicUsize,
    pub current: AtomicUsize,
    pub skipped: AtomicUsize,
    pub stripped: AtomicUsize,
    pub errors: AtomicUsize,
}

pub struct Processor {
    pub config: Arc<Config>,
    pub backup_mgr: BackupManager,
    pub dry_run: bool,
    pub verbose: bool,
    pub template_override: Option<String>,
    pub author: String,
    pub project: String,
}

impl Processor {
    pub fn new(
        config: Arc<Config>,
        root: &Path,
        backup_dir: PathBuf,
        dry_run: bool,
        verbose: bool,
        create_backups: bool,
        template_override: Option<String>,
    ) -> Self {
        let author = get_git_author().unwrap_or_else(|_| "unknown".into());
        let project = get_project_name(root);

        Self {
            config,
            backup_mgr: BackupManager::new(backup_dir, create_backups),
            dry_run,
            verbose,
            template_override,
            author,
            project,
        }
    }

    pub fn run_tag(&self, root: &Path, output_path: &Path) -> anyhow::Result<Stats> {
        let walker = Walker::new(
            root.to_path_buf(),
            Arc::clone(&self.config),
            output_path.to_path_buf(),
            self.backup_mgr.backup_dir.clone(),
        );

        let entries = walker.walk();
        let stats = Stats::default();

        entries.par_iter().for_each(|entry| {
            match self.tag_file(entry, root) {
                Ok(action) => match action {
                    TagResult::Tagged  => { stats.tagged.fetch_add(1, Ordering::Relaxed); }
                    TagResult::Updated => { stats.updated.fetch_add(1, Ordering::Relaxed); }
                    TagResult::Current => { stats.current.fetch_add(1, Ordering::Relaxed); }
                },
                Err(e) => {
                    stats.errors.fetch_add(1, Ordering::Relaxed);
                    eprintln!("{} {}: {}", "error".red(), entry.rel_path.display(), e);
                }
            }
        });

        Ok(stats)
    }

    pub fn run_strip(
        &self,
        root: &Path,
        output_path: &Path,
        strip_backup: bool,
    ) -> anyhow::Result<Stats> {
        let backup_dir = self.backup_mgr.backup_dir.clone();
        let walker = Walker::new(
            root.to_path_buf(),
            Arc::clone(&self.config),
            output_path.to_path_buf(),
            backup_dir.clone(),
        );

        let entries = walker.walk();
        let strip_mgr = BackupManager::new(backup_dir, strip_backup);
        let stats = Stats::default();

        entries.par_iter().for_each(|entry| {
            match self.strip_file(entry, root, &strip_mgr) {
                Ok(did_strip) => {
                    if did_strip {
                        stats.stripped.fetch_add(1, Ordering::Relaxed);
                    } else {
                        stats.current.fetch_add(1, Ordering::Relaxed);
                    }
                }
                Err(e) => {
                    stats.errors.fetch_add(1, Ordering::Relaxed);
                    eprintln!("{} {}: {}", "error".red(), entry.rel_path.display(), e);
                }
            }
        });

        Ok(stats)
    }

    /// Tag a single file by its absolute path (used by watcher).
    pub fn tag_file_by_path(&self, abs_path: &Path, root: &Path) -> anyhow::Result<()> {
        let rel_path = abs_path.strip_prefix(root).unwrap_or(abs_path).to_path_buf();
        let ext = abs_path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let style = match CommentStyle::from_ext(&ext) {
            Some(s) => s,
            None => return Ok(()),
        };
        let entry = WalkEntry { abs_path: abs_path.to_path_buf(), rel_path, style };
        self.tag_file(&entry, root)?;
        Ok(())
    }

    fn tag_file(&self, entry: &WalkEntry, root: &Path) -> anyhow::Result<TagResult> {
        let content = std::fs::read_to_string(&entry.abs_path)?;

        // Pick template: per-extension override → CLI override → config default
        let ext = entry.abs_path
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();
        let template = self.template_override
            .as_deref()
            .or_else(|| self.config.template.overrides.get(&ext).map(|s| s.as_str()))
            .unwrap_or(&self.config.template.default)
            .to_string();

        let ctx = TemplateContext::new(
            &entry.rel_path,
            &self.config.template.date_format,
            self.author.clone(),
            self.project.clone(),
            self.config.template.variables.clone(),
        );

        let desired = header::build_header(entry.style, &template, &ctx);

        match header::analyze(&content, &desired, entry.style) {
            HeaderAction::AlreadyCurrent => {
                if self.verbose {
                    println!("{} {}", "current".dimmed(), entry.rel_path.display());
                }
                return Ok(TagResult::Current);
            }
            HeaderAction::UpdateExisting | HeaderAction::AddNew => {
                let result_type = match header::analyze(&content, &desired, entry.style) {
                    HeaderAction::UpdateExisting => TagResult::Updated,
                    _ => TagResult::Tagged,
                };

                if self.dry_run {
                    let label = if matches!(result_type, TagResult::Updated) {
                        "would update".blue()
                    } else {
                        "would tag".purple()
                    };
                    println!("{} {}", label, entry.rel_path.display());
                    return Ok(result_type);
                }

                // Create backup
                self.backup_mgr.backup(&entry.abs_path, root)?;

                // Apply header
                let new_content = header::apply_tag(&content, &desired, entry.style);
                BackupManager::write_atomic(&entry.abs_path, &new_content)?;

                let label = if matches!(result_type, TagResult::Updated) {
                    "updated".blue()
                } else {
                    "tagged".purple()
                };
                if self.verbose {
                    println!("{} {}", label, entry.rel_path.display());
                }
                Ok(result_type)
            }
        }
    }

    fn strip_file(
        &self,
        entry: &WalkEntry,
        root: &Path,
        backup_mgr: &BackupManager,
    ) -> anyhow::Result<bool> {
        let content = std::fs::read_to_string(&entry.abs_path)?;

        match header::strip(&content, entry.style) {
            None => Ok(false), // no header found
            Some(new_content) => {
                if self.dry_run {
                    println!("{} {}", "would strip".yellow(), entry.rel_path.display());
                    return Ok(true);
                }
                backup_mgr.backup(&entry.abs_path, root)?;
                BackupManager::write_atomic(&entry.abs_path, &new_content)?;
                if self.verbose {
                    println!("{} {}", "stripped".yellow(), entry.rel_path.display());
                }
                Ok(true)
            }
        }
    }
}

enum TagResult {
    Tagged,
    Updated,
    Current,
}

fn get_git_author() -> anyhow::Result<String> {
    let out = std::process::Command::new("git")
        .args(["config", "user.name"])
        .output()?;
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if name.is_empty() {
        anyhow::bail!("empty git user.name");
    }
    Ok(name)
}

fn get_project_name(root: &Path) -> String {
    root.canonicalize()
        .unwrap_or_else(|_| root.to_path_buf())
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".into())
}
