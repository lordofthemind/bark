// File: src/lib.rs
pub mod backup;
pub mod cli;
pub mod config;
pub mod header;
pub mod processor;
pub mod template;
pub mod tree;
pub mod walker;
pub mod watcher;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};
use colored::Colorize;
use config::Config;
use processor::Processor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub fn run() -> Result<()> {
    run_with_cli(Cli::parse())
}

pub fn run_with_cli(cli: Cli) -> Result<()> {
    let command = cli
        .command
        .unwrap_or_else(|| Commands::Tag(cli::TagArgs::default()));

    match command {
        Commands::Tag(args) => {
            // Configure rayon thread pool once for all roots
            if args.threads > 0 {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(args.threads)
                    .build_global()
                    .ok();
            }

            let roots = resolve_roots(&args.roots);
            let multi = roots.len() > 1;

            for root_path in &roots {
                let root = root_path
                    .canonicalize()
                    .unwrap_or_else(|_| root_path.clone());

                let config = load_config(&cli.config, &root)?;
                let max_size = args.max_size.unwrap_or(config.general.max_file_size);
                let mut cfg = config;
                cfg.general.max_file_size = max_size;

                let backup_dir = root.join(&args.backup_dir);
                let output_path = root.join(&args.output);
                let create_backups = !args.force && cfg.general.backup;
                let config = Arc::new(cfg);

                if multi {
                    println!("\n{} {}", "→".bold(), root.display());
                }

                // Generate tree (skip in dry-run mode)
                if !args.no_tree {
                    if args.dry_run {
                        println!("{} would write {}", "tree".dimmed(), output_path.display());
                    } else if output_path.is_dir() {
                        eprintln!(
                            "{} '{}' is a directory — remove it or use --output to pick a different name",
                            "warn".yellow(),
                            output_path.display()
                        );
                    } else {
                        let gen = tree::TreeGenerator::new(
                            &root,
                            &backup_dir,
                            &output_path,
                            &config.exclude.patterns,
                        );
                        match gen.generate(&output_path) {
                            Ok(_) => {
                                if cli.verbose {
                                    println!(
                                        "{} tree written to {}",
                                        "bark".green().bold(),
                                        output_path.display()
                                    );
                                }
                            }
                            Err(e) => eprintln!("{} tree generation: {}", "warn".yellow(), e),
                        }
                    }
                }

                let proc = Processor::new(
                    Arc::clone(&config),
                    &root,
                    backup_dir,
                    args.dry_run,
                    cli.verbose,
                    create_backups,
                    args.template.clone(),
                );

                let stats = proc.run_tag(&root, &output_path)?;
                print_tag_summary(&stats, args.dry_run);
            }
        }

        Commands::Strip(args) => {
            let roots = resolve_roots(&args.roots);
            let multi = roots.len() > 1;

            for root_path in &roots {
                let root = root_path
                    .canonicalize()
                    .unwrap_or_else(|_| root_path.clone());

                let config = load_config(&cli.config, &root)?;
                let backup_dir = root.join(&args.backup_dir);
                let output_path = root.join(&config.general.output);
                let config = Arc::new(config);

                if multi {
                    println!("\n{} {}", "→".bold(), root.display());
                }

                let proc = Processor::new(
                    Arc::clone(&config),
                    &root,
                    backup_dir,
                    args.dry_run,
                    cli.verbose,
                    args.backup,
                    None,
                );

                let stats = proc.run_strip(&root, &output_path, args.backup)?;
                print_strip_summary(&stats, args.dry_run);
            }
        }

        Commands::Tree(args) => {
            let roots = resolve_roots(&args.roots);

            for root_path in &roots {
                let root = root_path
                    .canonicalize()
                    .unwrap_or_else(|_| root_path.clone());

                let config = load_config(&cli.config, &root)?;
                let output_path = root.join(&args.output);

                if output_path.is_dir() {
                    anyhow::bail!(
                        "'{}' is a directory — use --output to specify a different filename",
                        output_path.display()
                    );
                }

                let backup_dir = root.join(config.general.backup_dir.clone());
                let gen = tree::TreeGenerator::new(
                    &root,
                    &backup_dir,
                    &output_path,
                    &config.exclude.patterns,
                );
                gen.generate(&output_path)?;
                println!(
                    "{} tree written to {}",
                    "bark".green().bold(),
                    output_path.display()
                );
            }
        }

        Commands::Watch(args) => {
            let root = args
                .root
                .clone()
                .unwrap_or_else(|| PathBuf::from("."))
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from("."));

            let config = load_config(&cli.config, &root)?;
            let backup_dir = root.join(&config.general.backup_dir);
            let output_path = root.join(&args.output);
            let debounce = args.debounce;
            let config = Arc::new(config);

            let proc = Arc::new(Processor::new(
                Arc::clone(&config),
                &root,
                backup_dir,
                args.dry_run,
                cli.verbose,
                true,
                None,
            ));

            let fw = watcher::FileWatcher::new(proc, debounce, output_path, args.dry_run);
            fw.run(&root)?;
        }

        Commands::Restore(args) => {
            let root = args
                .root
                .clone()
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            let backup_dir = root.join(&args.backup_dir);
            let mgr = backup::BackupManager::new(backup_dir, false);

            let mut entries = mgr.list_backups(args.file.as_deref(), &root)?;

            if entries.is_empty() {
                println!("{} No backups found.", "bark".green().bold());
                return Ok(());
            }

            if args.latest {
                // Restore most recent backup per original file
                let mut seen = std::collections::HashSet::new();
                entries.retain(|e| seen.insert(e.original.clone()));
                for entry in &entries {
                    mgr.restore(entry, args.dry_run)?;
                    if !args.dry_run {
                        println!(
                            "{} restored: {}",
                            "bark".green().bold(),
                            entry.original.display()
                        );
                    }
                }
                return Ok(());
            }

            // Interactive: list backups and let user pick
            println!("{}", "Available backups:".bold());
            for (i, e) in entries.iter().enumerate() {
                println!(
                    "  [{:>2}] {} ({})",
                    i + 1,
                    e.original.display(),
                    e.timestamp.format("%Y-%m-%d %H:%M:%S")
                );
            }
            println!();
            print!("Enter number to restore (or 0 to cancel): ");
            use std::io::Write;
            std::io::stdout().flush()?;
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let choice: usize = input.trim().parse().unwrap_or(0);
            if choice == 0 || choice > entries.len() {
                println!("Cancelled.");
                return Ok(());
            }
            let entry = &entries[choice - 1];
            mgr.restore(entry, args.dry_run)?;
            if !args.dry_run {
                println!(
                    "{} restored: {}",
                    "bark".green().bold(),
                    entry.original.display()
                );
            }
        }

        Commands::Init(args) => {
            let dir = args.dir.unwrap_or_else(|| PathBuf::from("."));
            let target = dir.join(".bark.toml");
            if target.exists() && !args.force {
                anyhow::bail!(".bark.toml already exists — use --force to overwrite");
            }
            std::fs::write(&target, default_config_toml())?;
            println!("{} created {}", "bark".green().bold(), target.display());
        }
    }

    Ok(())
}

fn resolve_roots(roots: &[PathBuf]) -> Vec<PathBuf> {
    if roots.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        roots.to_vec()
    }
}

fn load_config(explicit: &Option<PathBuf>, root: &Path) -> Result<Config> {
    match explicit {
        Some(path) => Config::from_file(path),
        None => Ok(Config::find_and_load(root)?.unwrap_or_default()),
    }
}

pub fn print_tag_summary(stats: &processor::Stats, dry_run: bool) {
    use std::sync::atomic::Ordering;
    let mode = if dry_run { " (dry run)" } else { "" };
    println!();
    println!("{}{}", "bark done".green().bold(), mode);
    let tagged = stats.tagged.load(Ordering::Relaxed);
    let updated = stats.updated.load(Ordering::Relaxed);
    let current = stats.current.load(Ordering::Relaxed);
    let skipped = stats.skipped.load(Ordering::Relaxed);
    let errors = stats.errors.load(Ordering::Relaxed);
    let total = tagged + updated + current + skipped + errors;
    if total == 0 {
        println!(
            "  {} — run {} to debug, or {} to create a config",
            "no matching files found".yellow(),
            "bark tag -v".bold(),
            "bark init".bold()
        );
        return;
    }
    if tagged > 0 {
        println!("  {} tagged", tagged.to_string().purple());
    }
    if updated > 0 {
        println!("  {} updated", updated.to_string().blue());
    }
    if current > 0 {
        println!("  {} current", current.to_string().dimmed());
    }
    if skipped > 0 {
        println!("  {} skipped", skipped.to_string().dimmed());
    }
    if errors > 0 {
        println!("  {} errors", errors.to_string().red());
    }
}

pub fn print_strip_summary(stats: &processor::Stats, dry_run: bool) {
    use std::sync::atomic::Ordering;
    let mode = if dry_run { " (dry run)" } else { "" };
    println!();
    println!("{}{}", "bark strip done".yellow().bold(), mode);
    let stripped = stats.stripped.load(Ordering::Relaxed);
    let current = stats.current.load(Ordering::Relaxed);
    let errors = stats.errors.load(Ordering::Relaxed);
    if stripped > 0 {
        println!("  {} headers removed", stripped.to_string().yellow());
    }
    if current > 0 {
        println!("  {} files had no header", current.to_string().dimmed());
    }
    if errors > 0 {
        println!("  {} errors", errors.to_string().red());
    }
}

pub fn default_config_toml() -> &'static str {
    include_str!("default_config.toml")
}
