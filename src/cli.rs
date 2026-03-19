// File: src/cli.rs
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "bark",
    version,
    about = "bark — smart file header management and directory tree generator",
    long_about = "bark stamps standardized headers onto source files and generates a directory tree.\nRun `bark` in any project directory to get started."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Show detailed processing information
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Path to config file (default: searches upward for .bark.toml)
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Add or update file path headers [default when no subcommand given]
    Tag(TagArgs),
    /// Remove all bark-managed headers from files
    Strip(StripArgs),
    /// Generate the directory tree only — no headers are added or modified
    Tree(TreeArgs),
    /// Watch directory and auto-tag files on change
    Watch(WatchArgs),
    /// Restore files from a backup
    Restore(RestoreArgs),
    /// Create a .bark.toml config file in the current directory
    Init(InitArgs),
}

#[derive(Args, Debug, Default)]
pub struct TagArgs {
    /// Preview changes without modifying files
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Skip creating backups before modifying files
    #[arg(short, long)]
    pub force: bool,

    /// Output file for the generated directory tree
    #[arg(short, long, default_value = "tree.txt", value_name = "FILE")]
    pub output: PathBuf,

    /// Directory to store backups
    #[arg(short, long, default_value = ".bark_backups", value_name = "DIR")]
    pub backup_dir: PathBuf,

    /// Root directory to process (default: current directory)
    #[arg(value_name = "DIR")]
    pub root: Option<PathBuf>,

    /// Override header template for this run (e.g. "File: {{file}} | Author: {{author}}")
    #[arg(long, value_name = "TEMPLATE")]
    pub template: Option<String>,

    /// Maximum file size to process in bytes
    #[arg(long, value_name = "BYTES")]
    pub max_size: Option<u64>,

    /// Number of parallel threads (0 = automatic)
    #[arg(long, default_value = "0", value_name = "N")]
    pub threads: usize,

    /// Skip generating tree.txt
    #[arg(long)]
    pub no_tree: bool,
}

#[derive(Args, Debug)]
pub struct StripArgs {
    /// Preview which headers would be removed without modifying files
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Root directory to process (default: current directory)
    #[arg(value_name = "DIR")]
    pub root: Option<PathBuf>,

    /// Create backups before stripping
    #[arg(short, long)]
    pub backup: bool,

    /// Backup directory (used with --backup)
    #[arg(long, default_value = ".bark_backups", value_name = "DIR")]
    pub backup_dir: PathBuf,
}

#[derive(Args, Debug)]
pub struct WatchArgs {
    /// Root directory to watch (default: current directory)
    #[arg(value_name = "DIR")]
    pub root: Option<PathBuf>,

    /// Debounce delay in milliseconds before processing changes
    #[arg(long, default_value = "500", value_name = "MS")]
    pub debounce: u64,

    /// Log what would change without writing files
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Output file for tree regeneration on change
    #[arg(short, long, default_value = "tree.txt", value_name = "FILE")]
    pub output: PathBuf,
}

#[derive(Args, Debug)]
pub struct RestoreArgs {
    /// Root directory of the project (default: current directory)
    #[arg(long, value_name = "DIR")]
    pub root: Option<PathBuf>,

    /// Backup directory to restore from
    #[arg(default_value = ".bark_backups", value_name = "DIR")]
    pub backup_dir: PathBuf,

    /// Restore only backups for this specific file
    #[arg(long, value_name = "FILE")]
    pub file: Option<PathBuf>,

    /// Preview what would be restored without writing
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Automatically restore the most recent backup of each file
    #[arg(long)]
    pub latest: bool,
}

#[derive(Args, Debug)]
pub struct TreeArgs {
    /// Root directory to scan (default: current directory)
    #[arg(value_name = "DIR")]
    pub root: Option<PathBuf>,

    /// Output file for the generated directory tree
    #[arg(short, long, default_value = "tree.txt", value_name = "FILE")]
    pub output: PathBuf,
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Directory to create .bark.toml in (default: current directory)
    #[arg(value_name = "DIR")]
    pub dir: Option<PathBuf>,

    /// Overwrite existing .bark.toml if present
    #[arg(long)]
    pub force: bool,
}
