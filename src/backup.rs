// File: src/backup.rs
use anyhow::{Context, Result};
use chrono::{DateTime, Local, TimeZone};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

pub struct BackupManager {
    pub backup_dir: PathBuf,
    pub enabled: bool,
}

pub struct BackupEntry {
    pub original: PathBuf,
    pub backup_path: PathBuf,
    pub timestamp: DateTime<Local>,
}

impl BackupManager {
    pub fn new(backup_dir: PathBuf, enabled: bool) -> Self {
        Self {
            backup_dir,
            enabled,
        }
    }

    /// Create a timestamped backup of `file` before it is modified.
    pub fn backup(&self, file: &Path, root: &Path) -> Result<Option<PathBuf>> {
        if !self.enabled {
            return Ok(None);
        }
        let root_canon = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let file_canon = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
        let rel = file_canon.strip_prefix(&root_canon).unwrap_or(file);
        let ts = Local::now().format("%Y%m%d_%H%M%S").to_string();
        // Preserve directory structure inside backup_dir
        let backup_path = self.backup_dir.join(format!(
            "{}.{}.bak",
            rel.to_string_lossy().replace('\\', "/"),
            ts
        ));

        if let Some(parent) = backup_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating backup dir: {}", parent.display()))?;
        }
        std::fs::copy(file, &backup_path)
            .with_context(|| format!("backing up: {}", file.display()))?;
        Ok(Some(backup_path))
    }

    /// Atomically write `content` to `target`.
    /// Creates the tempfile in the same directory as the target so the
    /// rename (persist) is guaranteed to be on the same filesystem.
    pub fn write_atomic(target: &Path, content: &str) -> Result<()> {
        let parent = target
            .parent()
            .with_context(|| format!("no parent for: {}", target.display()))?;

        // Capture original permissions if the file already exists
        let original_perms = if target.exists() {
            Some(std::fs::metadata(target)?.permissions())
        } else {
            None
        };

        let mut tmp = NamedTempFile::new_in(parent)
            .with_context(|| format!("creating tempfile near: {}", target.display()))?;
        tmp.write_all(content.as_bytes())
            .with_context(|| "writing to tempfile")?;
        tmp.flush()?;

        // Restore permissions before rename
        if let Some(perms) = original_perms {
            std::fs::set_permissions(tmp.path(), perms)?;
        }

        tmp.persist(target)
            .with_context(|| format!("persisting to: {}", target.display()))?;
        Ok(())
    }

    /// List all backup entries in the backup directory.
    pub fn list_backups(
        &self,
        filter_file: Option<&Path>,
        root: &Path,
    ) -> Result<Vec<BackupEntry>> {
        if !self.backup_dir.exists() {
            return Ok(vec![]);
        }

        let filter_rel = filter_file
            .and_then(|f| f.strip_prefix(root).ok().or(Some(f)))
            .map(|p| p.to_string_lossy().replace('\\', "/"));

        let mut entries = vec![];
        self.collect_backups(&self.backup_dir, &mut entries, &filter_rel)?;
        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(entries)
    }

    fn collect_backups(
        &self,
        dir: &Path,
        out: &mut Vec<BackupEntry>,
        filter: &Option<String>,
    ) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.collect_backups(&path, out, filter)?;
            } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if !name.ends_with(".bak") {
                    continue;
                }
                // Parse: <rel_path>.<YYYYMMDD_HHMMSS>.bak
                if let Some(be) = parse_backup_entry(&path, &self.backup_dir) {
                    if let Some(ref f) = filter {
                        let orig = be.original.to_string_lossy().replace('\\', "/");
                        if !orig.contains(f.as_str()) {
                            continue;
                        }
                    }
                    out.push(be);
                }
            }
        }
        Ok(())
    }

    pub fn restore(&self, entry: &BackupEntry, dry_run: bool) -> Result<()> {
        if dry_run {
            println!(
                "  would restore: {} ← {}",
                entry.original.display(),
                entry.backup_path.display()
            );
            return Ok(());
        }
        if let Some(parent) = entry.original.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&entry.backup_path, &entry.original)
            .with_context(|| format!("restoring: {}", entry.original.display()))?;
        Ok(())
    }
}

fn parse_backup_entry(backup_path: &Path, backup_dir: &Path) -> Option<BackupEntry> {
    let rel_to_dir = backup_path.strip_prefix(backup_dir).ok()?;
    let name = rel_to_dir.to_string_lossy();
    // Remove .bak
    let without_bak = name.strip_suffix(".bak")?;
    // The timestamp is the last 15 chars: YYYYMMDD_HHMMSS
    if without_bak.len() < 16 {
        return None;
    }
    let (orig_part, ts_part) = without_bak.split_at(without_bak.len() - 15 - 1);
    // ts_part starts with '.'
    let ts_str = ts_part.strip_prefix('.')?;
    let naive = chrono::NaiveDateTime::parse_from_str(ts_str, "%Y%m%d_%H%M%S").ok()?;
    // The filename timestamp is written in local time, so interpret it as local, not UTC.
    let timestamp: DateTime<Local> = chrono::Local
        .from_local_datetime(&naive)
        .single()
        .unwrap_or_else(Local::now);

    Some(BackupEntry {
        original: PathBuf::from(orig_part),
        backup_path: backup_path.to_path_buf(),
        timestamp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_backup_entry_valid() {
        let tmp = tempfile::TempDir::new().unwrap();
        let backup_dir = tmp.path().to_path_buf();

        // Create a fake backup file with the expected naming convention
        let fake_backup = backup_dir.join("src/main.rs.20260319_142022.bak");
        std::fs::create_dir_all(fake_backup.parent().unwrap()).unwrap();
        std::fs::write(&fake_backup, "content").unwrap();

        let entry = parse_backup_entry(&fake_backup, &backup_dir);
        assert!(entry.is_some(), "should parse a valid backup filename");
        let e = entry.unwrap();
        assert_eq!(e.original, PathBuf::from("src/main.rs"));
    }

    #[test]
    fn parse_backup_entry_invalid() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bad = tmp.path().join("not_a_backup.txt");
        std::fs::write(&bad, "x").unwrap();
        assert!(parse_backup_entry(&bad, tmp.path()).is_none());
    }

    #[test]
    fn write_atomic_creates_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let target = tmp.path().join("output.txt");
        BackupManager::write_atomic(&target, "hello bark\n").unwrap();
        let content = std::fs::read_to_string(&target).unwrap();
        assert_eq!(content, "hello bark\n");
    }

    #[test]
    fn write_atomic_overwrites_existing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let target = tmp.path().join("output.txt");
        std::fs::write(&target, "old content\n").unwrap();
        BackupManager::write_atomic(&target, "new content\n").unwrap();
        let content = std::fs::read_to_string(&target).unwrap();
        assert_eq!(content, "new content\n");
    }

    #[test]
    fn list_backups_filter_by_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let backup_dir = root.join(".bark_backups");

        // Create two fake backup files
        std::fs::create_dir_all(&backup_dir).unwrap();
        std::fs::write(
            backup_dir.join("src/main.rs.20260319_120000.bak"),
            "main content",
        )
        .unwrap_or_else(|_| {
            std::fs::create_dir_all(backup_dir.join("src")).unwrap();
            std::fs::write(
                backup_dir.join("src/main.rs.20260319_120000.bak"),
                "main content",
            )
            .unwrap();
        });
        std::fs::create_dir_all(backup_dir.join("src")).unwrap();
        std::fs::write(
            backup_dir.join("src/main.rs.20260319_120000.bak"),
            "main content",
        )
        .unwrap();
        std::fs::write(backup_dir.join("lib.rs.20260319_120001.bak"), "lib content").unwrap();

        let mgr = BackupManager::new(backup_dir, false);
        let all = mgr.list_backups(None, &root).unwrap();
        assert_eq!(all.len(), 2, "should find both backups");

        let filtered = mgr
            .list_backups(Some(std::path::Path::new("main.rs")), &root)
            .unwrap();
        assert_eq!(filtered.len(), 1, "filter should find only main.rs backup");
    }

    #[test]
    fn restore_copies_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let backup_dir = tmp.path().join(".bark_backups");
        std::fs::create_dir_all(&backup_dir).unwrap();

        let bak = backup_dir.join("out.rs.20260319_120000.bak");
        std::fs::write(&bak, "backup content\n").unwrap();
        let dest = tmp.path().join("out.rs");

        // Build a BackupEntry with an *absolute* original path so restore() writes to the right place
        let entry = BackupEntry {
            original: dest.clone(),
            backup_path: bak,
            timestamp: chrono::Local::now(),
        };

        let mgr = BackupManager::new(backup_dir, false);
        mgr.restore(&entry, false).unwrap();

        assert!(dest.exists(), "restored file should exist");
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "backup content\n");
    }

    #[test]
    fn restore_creates_parent_dirs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let backup_dir = tmp.path().join(".bark_backups");
        std::fs::create_dir_all(&backup_dir).unwrap();

        let bak = backup_dir.join("source.rs.20260319_120000.bak");
        std::fs::write(&bak, "content\n").unwrap();
        // Destination in a subdirectory that doesn't yet exist
        let dest = tmp.path().join("nested").join("dir").join("source.rs");

        let entry = BackupEntry {
            original: dest.clone(),
            backup_path: bak,
            timestamp: chrono::Local::now(),
        };

        let mgr = BackupManager::new(backup_dir, false);
        mgr.restore(&entry, false).unwrap();
        assert!(dest.exists());
    }

    #[test]
    fn parse_backup_entry_too_short() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Filename ends in .bak but the stem is too short to contain a timestamp
        let bad = tmp.path().join("a.bak");
        std::fs::write(&bad, "x").unwrap();
        assert!(parse_backup_entry(&bad, tmp.path()).is_none());
    }

    #[test]
    fn backup_disabled_returns_none() {
        let tmp = tempfile::TempDir::new().unwrap();
        let source = tmp.path().join("main.rs");
        std::fs::write(&source, "content").unwrap();

        let mgr = BackupManager::new(tmp.path().join(".bark_backups"), false);
        let result = mgr.backup(&source, tmp.path()).unwrap();
        assert!(result.is_none(), "disabled backup should return None");
    }

    #[test]
    fn backup_creates_file_in_backup_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let source = root.join("src/main.rs");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(&source, "original content\n").unwrap();

        let backup_dir = root.join(".bark_backups");
        let mgr = BackupManager::new(backup_dir.clone(), true);
        let result = mgr.backup(&source, &root).unwrap();

        assert!(result.is_some());
        let backup_path = result.unwrap();
        assert!(backup_path.exists());
        let content = std::fs::read_to_string(&backup_path).unwrap();
        assert_eq!(content, "original content\n");
    }
}
