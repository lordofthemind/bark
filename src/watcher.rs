// File: src/watcher.rs
use crate::processor::Processor;
use crate::tree::TreeGenerator;
use colored::Colorize;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct FileWatcher {
    processor: Arc<Processor>,
    debounce_ms: u64,
    output_path: PathBuf,
    dry_run: bool,
}

impl FileWatcher {
    pub fn new(
        processor: Arc<Processor>,
        debounce_ms: u64,
        output_path: PathBuf,
        dry_run: bool,
    ) -> Self {
        Self { processor, debounce_ms, output_path, dry_run }
    }

    pub fn run(&self, root: &Path) -> anyhow::Result<()> {
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
        let recently_written: Arc<Mutex<HashSet<PathBuf>>> =
            Arc::new(Mutex::new(HashSet::new()));

        loop {
            // Block until we get the first event
            let first = match rx.recv() {
                Ok(e) => e,
                Err(_) => break,
            };

            let Ok(first_event) = first else { continue };

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
                paths
                    .into_iter()
                    .filter(|p| !rw.remove(p))
                    .collect()
            };

            if paths.is_empty() {
                continue;
            }

            let mut processed = 0usize;
            for path in &paths {
                if self.dry_run {
                    println!("{} {}", "would tag".purple(), path.display());
                    processed += 1;
                    continue;
                }
                // Mark as recently written before we write
                {
                    recently_written.lock().unwrap().insert(path.clone());
                }
                match self.processor.tag_file_by_path(path, root) {
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
                let gen = TreeGenerator::new(root, &backup_dir, &self.output_path);
                if let Err(e) = gen.generate(&self.output_path) {
                    eprintln!("{} regenerating tree: {}", "warn".yellow(), e);
                }
                println!(
                    "{} processed {} file(s)",
                    "bark".green().bold(),
                    processed
                );
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
