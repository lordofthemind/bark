// File: src/watcher.rs
use crate::processor::Processor;
use crate::tree::TreeGenerator;
use colored::Colorize;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct FileWatcher {
    pub processor: Arc<Processor>,
    pub debounce_ms: u64,
    pub output_path: PathBuf,
    pub dry_run: bool,
}

impl FileWatcher {
    pub fn new(
        processor: Arc<Processor>,
        debounce_ms: u64,
        output_path: PathBuf,
        dry_run: bool,
    ) -> Self {
        Self {
            processor,
            debounce_ms,
            output_path,
            dry_run,
        }
    }

    /// Run forever (normal CLI use).
    pub fn run(&self, root: &Path) -> anyhow::Result<()> {
        self.run_until_stopped(root, None)
    }

    /// Run until `stop` is set to true (useful for tests).
    /// The loop checks the flag every 100 ms when idle.
    pub fn run_until_stopped(
        &self,
        root: &Path,
        stop: Option<Arc<AtomicBool>>,
    ) -> anyhow::Result<()> {
        println!(
            "{} Watching {} for changes… (Ctrl-C to stop)",
            "bark".green().bold(),
            root.display()
        );

        let (tx, rx) = std::sync::mpsc::channel::<notify::Result<Event>>();

        let mut watcher = RecommendedWatcher::new(
            move |event: notify::Result<Event>| {
                tx.send(event).ok();
            },
            notify::Config::default(),
        )?;

        watcher.watch(root, RecursiveMode::Recursive)?;

        // Track files bark itself just wrote so we don't re-process them
        let recently_written: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));

        loop {
            // Check stop flag first
            if stop.as_ref().is_some_and(|s| s.load(Ordering::Relaxed)) {
                break;
            }

            // Wait for an event with a short timeout so we can poll the stop flag
            let first_event = match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(Ok(e)) => e,
                Ok(Err(_)) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            };

            // Collect additional events within the debounce window
            let deadline = Instant::now() + Duration::from_millis(self.debounce_ms);
            let mut events = vec![first_event];

            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                match rx.recv_timeout(remaining) {
                    Ok(Ok(e)) => events.push(e),
                    _ => break,
                }
            }

            // Collect paths that need processing
            let mut paths: HashSet<PathBuf> = HashSet::new();
            for event in &events {
                if is_write_event(&event.kind) {
                    for p in &event.paths {
                        if p.is_file() {
                            let canon = p.canonicalize().unwrap_or_else(|_| p.clone());
                            paths.insert(canon);
                        }
                    }
                }
            }

            if paths.is_empty() {
                continue;
            }

            // Filter out files we just wrote
            let paths: Vec<PathBuf> = {
                let mut rw = recently_written.lock().unwrap();
                paths.into_iter().filter(|p| !rw.remove(p)).collect()
            };

            if paths.is_empty() {
                continue;
            }

            let mut processed = 0usize;
            for path in &paths {
                // Respect [watch] ignore patterns
                if !self.processor.config.watch.ignore.is_empty() {
                    if let Ok(rel) = path.strip_prefix(root) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        if crate::walker::is_path_excluded(
                            &rel_str,
                            &self.processor.config.watch.ignore,
                        ) {
                            continue;
                        }
                    }
                }
                let abs_path = path;
                if self.dry_run {
                    println!("{} {}", "would tag".purple(), abs_path.display());
                    processed += 1;
                    continue;
                }
                // Mark as recently written before we write
                {
                    recently_written.lock().unwrap().insert(abs_path.clone());
                }
                match self.processor.tag_file_by_path(abs_path, root) {
                    Ok(_) => processed += 1,
                    Err(e) => {
                        eprintln!("{} {}: {}", "error".red(), path.display(), e);
                        // Remove from set since we didn't actually write it
                        recently_written.lock().unwrap().remove(path);
                    }
                }
            }

            // Regenerate tree if any files were processed
            if processed > 0 && !self.dry_run {
                let backup_dir = self.processor.backup_mgr.backup_dir.clone();
                let gen = TreeGenerator::new(
                    root,
                    &backup_dir,
                    &self.output_path,
                    &self.processor.config.exclude.patterns,
                );
                if let Err(e) = gen.generate(&self.output_path) {
                    eprintln!("{} regenerating tree: {}", "warn".yellow(), e);
                }
                println!("{} processed {} file(s)", "bark".green().bold(), processed);
            }

            // Check stop after processing a batch
            if stop.as_ref().is_some_and(|s| s.load(Ordering::Relaxed)) {
                break;
            }
        }

        Ok(())
    }

    pub fn run_multi(&self, roots: &[(PathBuf, Arc<Processor>)]) -> anyhow::Result<()> {
        self.run_multi_until_stopped(roots, None)
    }

    pub fn run_multi_until_stopped(
        &self,
        roots: &[(PathBuf, Arc<Processor>)],
        stop: Option<Arc<AtomicBool>>,
    ) -> anyhow::Result<()> {
        let root_strs: Vec<String> = roots.iter().map(|(r, _)| r.display().to_string()).collect();
        println!(
            "{} Watching {} for changes… (Ctrl-C to stop)",
            "bark".green().bold(),
            root_strs.join(", ")
        );

        let (tx, rx) = std::sync::mpsc::channel::<notify::Result<Event>>();
        let mut watcher = RecommendedWatcher::new(
            move |event: notify::Result<Event>| {
                tx.send(event).ok();
            },
            notify::Config::default(),
        )?;

        for (root, _) in roots {
            watcher.watch(root, RecursiveMode::Recursive)?;
        }

        let recently_written: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));
        let debounce = Duration::from_millis(self.debounce_ms);

        loop {
            if let Some(ref stop) = stop {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
            }

            let first = match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(ev) => ev,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break,
            };

            let mut batch = vec![first];
            let deadline = Instant::now() + debounce;
            while Instant::now() < deadline {
                match rx.recv_timeout(deadline - Instant::now()) {
                    Ok(ev) => batch.push(ev),
                    Err(_) => break,
                }
            }

            // Collect unique changed paths
            let mut changed: HashSet<PathBuf> = HashSet::new();
            for event in batch.into_iter().flatten() {
                if is_write_event(&event.kind) {
                    for path in event.paths {
                        if let Ok(c) = path.canonicalize() {
                            changed.insert(c);
                        } else {
                            changed.insert(path);
                        }
                    }
                }
            }

            // Remove recently written
            {
                let rw = recently_written.lock().unwrap();
                changed.retain(|p| !rw.contains(p));
            }

            if changed.is_empty() {
                continue;
            }

            // Process each changed file with the matching root's processor
            let mut roots_to_regen: HashSet<usize> = HashSet::new();
            for abs_path in &changed {
                // Find which root this file belongs to
                let root_idx = roots.iter().position(|(r, _)| abs_path.starts_with(r));
                let (root, proc) = match root_idx {
                    Some(i) => &roots[i],
                    None => continue,
                };

                // Apply watch ignore patterns
                if !proc.config.watch.ignore.is_empty() {
                    if let Ok(rel) = abs_path.strip_prefix(root) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        if crate::walker::is_path_excluded(&rel_str, &proc.config.watch.ignore) {
                            continue;
                        }
                    }
                }

                {
                    let mut rw = recently_written.lock().unwrap();
                    rw.insert(abs_path.clone());
                }

                if !self.dry_run {
                    proc.tag_file_by_path(abs_path, root).ok();
                    if let Some(i) = root_idx {
                        roots_to_regen.insert(i);
                    }
                } else {
                    println!("{} would tag: {}", "bark".dimmed(), abs_path.display());
                }
            }

            // Clear recently written after batch
            {
                let mut rw = recently_written.lock().unwrap();
                rw.clear();
            }

            // Regenerate tree for affected roots
            if !self.dry_run {
                for i in roots_to_regen {
                    let (root, proc) = &roots[i];
                    let output_path = root.join(&proc.config.general.output);
                    if !output_path.is_dir() {
                        let backup_dir = root.join(&proc.config.general.backup_dir);
                        let gen = TreeGenerator::new(
                            root,
                            &backup_dir,
                            &output_path,
                            &proc.config.exclude.patterns,
                        );
                        gen.generate(&output_path).ok();
                    }
                }
            }
        }
        Ok(())
    }
}

fn is_write_event(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(notify::event::ModifyKind::Data(_))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, DataChange, MetadataKind, ModifyKind, RemoveKind};

    #[test]
    fn is_write_event_create_is_true() {
        assert!(is_write_event(&EventKind::Create(CreateKind::File)));
        assert!(is_write_event(&EventKind::Create(CreateKind::Any)));
    }

    #[test]
    fn is_write_event_modify_data_is_true() {
        assert!(is_write_event(&EventKind::Modify(ModifyKind::Data(
            DataChange::Content
        ))));
        assert!(is_write_event(&EventKind::Modify(ModifyKind::Data(
            DataChange::Any
        ))));
    }

    #[test]
    fn is_write_event_remove_is_false() {
        assert!(!is_write_event(&EventKind::Remove(RemoveKind::File)));
    }

    #[test]
    fn is_write_event_other_modify_is_false() {
        assert!(!is_write_event(&EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::Any
        ))));
    }

    #[test]
    fn file_watcher_new() {
        use crate::config::Config;
        use crate::processor::Processor;
        use std::sync::Arc;

        let config = Arc::new(Config::default());
        let tmp = tempfile::TempDir::new().unwrap();
        let proc = Arc::new(Processor::new(
            config,
            tmp.path(),
            tmp.path().join(".barks"),
            false,
            false,
            false,
            None,
        ));
        let fw = FileWatcher::new(proc, 500, tmp.path().join("tree.txt"), false);
        assert_eq!(fw.debounce_ms, 500);
        assert!(!fw.dry_run);
    }
}
