// File: src/main.rs
mod backup;
mod cli;
mod config;
mod header;
mod processor;
mod template;
mod tree;
mod walker;
mod watcher;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};
use colored::Colorize;
use config::Config;
use processor::Processor;
use std::path::PathBuf;
use std::sync::Arc;

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load config: explicit flag → search upward → defaults
    let config = match &cli.config {
        Some(path) => Config::from_file(path)?,
        None => Config::find_and_load(&std::env::current_dir()?)?
            .unwrap_or_default(),
    };

    let command = cli.command.unwrap_or_else(|| Commands::Tag(cli::TagArgs::default()));

    match command {
        Commands::Tag(args) => {
            let root = args.root.clone()
                .unwrap_or_else(|| PathBuf::from("."))
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from("."));

            // Configure rayon thread pool
            if args.threads > 0 {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(args.threads)
                    .build_global()
                    .ok();
            }

            let max_size = args.max_size.unwrap_or(config.general.max_file_size);
            let mut cfg = config.clone();
            cfg.general.max_file_size = max_size;

            let backup_dir = root.join(&args.backup_dir);
            let output_path = root.join(&args.output);
            let create_backups = !args.force && cfg.general.backup;

            let config = Arc::new(cfg);

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
                    let gen = tree::TreeGenerator::new(&root, &backup_dir, &output_path);
                    match gen.generate(&output_path) {
                        Ok(_) => {
                            if cli.verbose {
                                println!("{} tree written to {}", "bark".green().bold(), output_path.display());
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
                args.template,
            );

            let stats = proc.run_tag(&root, &output_path)?;

            print_tag_summary(&stats, args.dry_run);
        }

        Commands::Strip(args) => {
            let root = args.root.clone()
                .unwrap_or_else(|| PathBuf::from("."))
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from("."));

            let backup_dir = root.join(&args.backup_dir);
            let output_path = root.join(&config.general.output);
            let config = Arc::new(config);

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

        Commands::Watch(args) => {
            let root = args.root.clone()
                .unwrap_or_else(|| PathBuf::from("."))
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from("."));

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

            let fw = watcher::FileWatcher::new(
                proc,
                debounce,
                output_path,
                args.dry_run,
            );
            fw.run(&root)?;
        }

        Commands::Restore(args) => {
            let root = std::env::current_dir()?;
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
                        println!("{} restored: {}", "bark".green().bold(), entry.original.display());
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
                println!("{} restored: {}", "bark".green().bold(), entry.original.display());
            }
        }

        Commands::Init(args) => {
            let target = PathBuf::from(".bark.toml");
            if target.exists() && !args.force {
                eprintln!(
                    "{} .bark.toml already exists. Use --force to overwrite.",
                    "error".red()
                );
                std::process::exit(1);
            }
            std::fs::write(&target, default_config_toml())?;
            println!("{} created .bark.toml", "bark".green().bold());
        }
    }

    Ok(())
}

fn print_tag_summary(stats: &processor::Stats, dry_run: bool) {
    use std::sync::atomic::Ordering;
    let mode = if dry_run { " (dry run)" } else { "" };
    println!();
    println!("{}{}", "bark done".green().bold(), mode);
    let tagged  = stats.tagged.load(Ordering::Relaxed);
    let updated = stats.updated.load(Ordering::Relaxed);
    let current = stats.current.load(Ordering::Relaxed);
    let skipped = stats.skipped.load(Ordering::Relaxed);
    let errors  = stats.errors.load(Ordering::Relaxed);
    let total = tagged + updated + current + skipped + errors;
    if total == 0 {
        println!("  {} — add extensions in .bark.toml, or run {} to debug", "no matching files found".yellow(), "bark tag -v".bold());
        return;
    }
    if tagged  > 0 { println!("  {} tagged",   tagged.to_string().purple()); }
    if updated > 0 { println!("  {} updated",  updated.to_string().blue()); }
    if current > 0 { println!("  {} current",  current.to_string().dimmed()); }
    if skipped > 0 { println!("  {} skipped",  skipped.to_string().dimmed()); }
    if errors  > 0 { println!("  {} errors",   errors.to_string().red()); }
}

fn print_strip_summary(stats: &processor::Stats, dry_run: bool) {
    use std::sync::atomic::Ordering;
    let mode = if dry_run { " (dry run)" } else { "" };
    println!();
    println!("{}{}", "bark strip done".yellow().bold(), mode);
    let stripped = stats.stripped.load(Ordering::Relaxed);
    let current  = stats.current.load(Ordering::Relaxed);
    let errors   = stats.errors.load(Ordering::Relaxed);
    if stripped > 0 { println!("  {} headers removed", stripped.to_string().yellow()); }
    if current  > 0 { println!("  {} files had no header", current.to_string().dimmed()); }
    if errors   > 0 { println!("  {} errors", errors.to_string().red()); }
}

fn default_config_toml() -> &'static str {
    r#"# .bark.toml — project configuration for bark

[general]
output      = "tree.txt"        # tree output filename
backup_dir  = ".bark_backups"   # where backups are stored
max_file_size = 1048576         # skip files larger than this (bytes)
backup      = true              # create backups before modifying files

[template]
# Available variables: {{file}}, {{date}}, {{year}}, {{author}}, {{project}}, {{filename}}, {{ext}}
default     = "File: {{file}}"
date_format = "%Y-%m-%d"

# Per-extension template overrides (extension without the dot)
[template.overrides]
# rs = "File: {{file}} | Author: {{author}} | {{date}}"
# py = "File: {{file}} | Project: {{project}}"

# Static variables available in all templates
[template.variables]
# author  = "Your Name"
# project = "my-project"

[exclude]
# Glob patterns for files/directories to skip
patterns = [
    "*.min.*",
    "*.bundle.*",
    "dist/**",
    "build/**",
    "node_modules/**",
    "vendor/**",
    "target/**",
]

[extensions]
# Extra extensions to process beyond the built-in set
# style: "slash" (// ...), "hash" (# ...), "css" (/* ... */), "html" (<!-- ... -->)
custom = [
    # { ext = "lua",    style = "slash" },
    # { ext = "svelte", style = "html"  },
]
# Extensions to always skip
skip = []

[watch]
debounce_ms = 500   # milliseconds to wait after a change before processing
ignore      = []    # additional glob patterns to ignore in watch mode
"#
}
