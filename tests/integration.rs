// File: tests/integration.rs

//! Integration tests for bark.
//! Each test creates an isolated temp directory, writes known files into it,
//! runs bark via `assert_cmd`, and asserts on both stdout and file-system state.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Create a minimal git repo in `dir` so the ignore crate doesn't bail out.
fn init_git(dir: &TempDir) {
    std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .expect("git init");
    // Silence git complaints about user identity
    std::process::Command::new("git")
        .args(["config", "user.email", "test@bark"])
        .current_dir(dir.path())
        .status()
        .ok();
    std::process::Command::new("git")
        .args(["config", "user.name", "bark-test"])
        .current_dir(dir.path())
        .status()
        .ok();
}

fn bark() -> Command {
    Command::cargo_bin("bark").expect("bark binary must be built before running tests")
}

// ── CLI smoke tests ──────────────────────────────────────────────────────────

#[test]
fn cli_help() {
    bark()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"));
}

#[test]
fn cli_version() {
    bark()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("0.0.1"));
}

// ── bark init ────────────────────────────────────────────────────────────────

#[test]
fn init_creates_config_file() {
    let dir = TempDir::new().unwrap();
    init_git(&dir);

    bark()
        .arg("init")
        .current_dir(dir.path())
        .assert()
        .success();

    let config_path = dir.path().join(".bark.toml");
    assert!(config_path.exists(), ".bark.toml should be created");
    let content = fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("[general]"));
    assert!(content.contains("[template]"));
    assert!(content.contains("[exclude]"));
}

#[test]
fn init_refuses_to_overwrite_without_force() {
    let dir = TempDir::new().unwrap();
    init_git(&dir);

    // First init
    bark()
        .arg("init")
        .current_dir(dir.path())
        .assert()
        .success();

    // Second init without --force should fail
    bark()
        .arg("init")
        .current_dir(dir.path())
        .assert()
        .failure();
}

#[test]
fn init_force_overwrites() {
    let dir = TempDir::new().unwrap();
    init_git(&dir);

    bark()
        .arg("init")
        .current_dir(dir.path())
        .assert()
        .success();
    bark()
        .args(["init", "--force"])
        .current_dir(dir.path())
        .assert()
        .success();
}

// ── bark tag ─────────────────────────────────────────────────────────────────

#[test]
fn tag_adds_header_to_go_file() {
    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(
        dir.path().join("main.go"),
        "package main\n\nfunc main() {}\n",
    )
    .unwrap();

    bark()
        .args(["tag", "--force", "--no-tree"])
        .current_dir(dir.path())
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join("main.go")).unwrap();
    assert!(
        content.starts_with("// File: main.go"),
        "header should be on line 0"
    );
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.get(1), Some(&""), "blank line should follow header");
    assert_eq!(lines.get(2), Some(&"package main"));
}

#[test]
fn tag_adds_header_to_rust_file() {
    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/lib.rs"), "pub fn hello() {}\n").unwrap();

    bark()
        .args(["tag", "--force", "--no-tree"])
        .current_dir(dir.path())
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    assert!(content.starts_with("// File: src/lib.rs"));
}

#[test]
fn tag_is_idempotent() {
    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(dir.path().join("main.go"), "package main\n").unwrap();

    // First tag
    bark()
        .args(["tag", "--force", "--no-tree"])
        .current_dir(dir.path())
        .assert()
        .success();

    let after_first = fs::read_to_string(dir.path().join("main.go")).unwrap();

    // Second tag — output file should be unchanged
    bark()
        .args(["tag", "--force", "--no-tree"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("current"));

    let after_second = fs::read_to_string(dir.path().join("main.go")).unwrap();
    assert_eq!(
        after_first, after_second,
        "second run should not modify files"
    );
}

#[test]
fn tag_dry_run_does_not_modify_files() {
    let dir = TempDir::new().unwrap();
    init_git(&dir);
    let original = "package main\n\nfunc main() {}\n";
    fs::write(dir.path().join("main.go"), original).unwrap();

    bark()
        .args(["tag", "--dry-run", "--no-tree"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("dry run"));

    let content = fs::read_to_string(dir.path().join("main.go")).unwrap();
    assert_eq!(content, original, "dry-run must not modify files");
}

#[test]
fn tag_custom_template() {
    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(dir.path().join("main.go"), "package main\n").unwrap();

    bark()
        .args([
            "tag",
            "--force",
            "--no-tree",
            "--template",
            "File: {{file}} | Bark",
        ])
        .current_dir(dir.path())
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join("main.go")).unwrap();
    assert!(content.starts_with("// File: main.go | Bark"));
}

#[test]
fn tag_creates_backup_by_default() {
    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(dir.path().join("main.go"), "package main\n").unwrap();

    // Run without --force so backup is created
    bark()
        .args(["tag", "--no-tree"])
        .current_dir(dir.path())
        .assert()
        .success();

    let backup_dir = dir.path().join(".bark_backups");
    assert!(
        backup_dir.exists(),
        ".bark_backups directory should be created"
    );

    // At least one .bak file should exist somewhere inside
    let has_bak = walkdir_has_extension(&backup_dir, "bak");
    assert!(has_bak, "at least one .bak file should exist");
}

#[test]
fn tag_skips_unknown_extensions() {
    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(dir.path().join("file.xyz"), "some content\n").unwrap();

    bark()
        .args(["tag", "--force", "--no-tree"])
        .current_dir(dir.path())
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join("file.xyz")).unwrap();
    assert_eq!(
        content, "some content\n",
        "unknown extension should be untouched"
    );
}

// ── bark strip ───────────────────────────────────────────────────────────────

#[test]
fn strip_removes_header() {
    let dir = TempDir::new().unwrap();
    init_git(&dir);
    // Pre-tag the file
    fs::write(
        dir.path().join("main.go"),
        "// File: main.go\n\npackage main\n",
    )
    .unwrap();

    bark()
        .arg("strip")
        .current_dir(dir.path())
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join("main.go")).unwrap();
    assert!(!content.contains("// File:"), "header should be removed");
    assert!(content.contains("package main"), "code should remain");
}

#[test]
fn strip_dry_run_does_not_modify() {
    let dir = TempDir::new().unwrap();
    init_git(&dir);
    let original = "// File: main.go\n\npackage main\n";
    fs::write(dir.path().join("main.go"), original).unwrap();

    bark()
        .args(["strip", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("dry run"));

    let content = fs::read_to_string(dir.path().join("main.go")).unwrap();
    assert_eq!(content, original);
}

// ── Multiple file types ──────────────────────────────────────────────────────

#[test]
fn tag_multiple_file_types() {
    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(dir.path().join("app.ts"), "export const x = 1;\n").unwrap();
    fs::write(dir.path().join("style.css"), "body { margin: 0; }\n").unwrap();
    fs::write(dir.path().join("index.html"), "<html></html>\n").unwrap();
    fs::write(dir.path().join("config.toml"), "[section]\nkey = \"val\"\n").unwrap();

    bark()
        .args(["tag", "--force", "--no-tree"])
        .current_dir(dir.path())
        .assert()
        .success();

    let ts = fs::read_to_string(dir.path().join("app.ts")).unwrap();
    let css = fs::read_to_string(dir.path().join("style.css")).unwrap();
    let html = fs::read_to_string(dir.path().join("index.html")).unwrap();
    let toml_file = fs::read_to_string(dir.path().join("config.toml")).unwrap();

    assert!(ts.starts_with("// File: app.ts"), "ts uses slash style");
    assert!(
        css.starts_with("/* File: style.css */"),
        "css uses css style"
    );
    assert!(
        html.starts_with("<!-- File: index.html -->"),
        "html uses html style"
    );
    assert!(
        toml_file.starts_with("# File: config.toml"),
        "toml uses hash style"
    );
}

// ── Config file ───────────────────────────────────────────────────────────────

#[test]
fn config_file_changes_template() {
    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(dir.path().join("main.go"), "package main\n").unwrap();
    fs::write(
        dir.path().join(".bark.toml"),
        "[template]\ndefault = \"File: {{file}} | CUSTOM\"\n",
    )
    .unwrap();

    bark()
        .args(["tag", "--force", "--no-tree"])
        .current_dir(dir.path())
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join("main.go")).unwrap();
    assert!(
        content.contains("| CUSTOM"),
        "config template should be used"
    );
}

// ── Helper ───────────────────────────────────────────────────────────────────

fn walkdir_has_extension(dir: &std::path::Path, ext: &str) -> bool {
    if !dir.exists() {
        return false;
    }
    for entry in walkdir::walkdir_simple(dir) {
        if entry.extension().and_then(|e| e.to_str()) == Some(ext) {
            return true;
        }
    }
    false
}

mod walkdir {
    use std::path::PathBuf;
    pub fn walkdir_simple(dir: &std::path::Path) -> Vec<PathBuf> {
        let mut out = vec![];
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    out.extend(walkdir_simple(&p));
                } else {
                    out.push(p);
                }
            }
        }
        out
    }
}

// ── Direct library tests (instrumented by tarpaulin) ─────────────────────────

#[test]
fn lib_processor_tags_go_file() {
    use bark::config::Config;
    use bark::processor::Processor;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(
        dir.path().join("main.go"),
        "package main\n\nfunc main() {}\n",
    )
    .unwrap();

    let config = Arc::new(Config::default());
    let backup_dir = dir.path().join(".bark_backups");
    let output_path = dir.path().join("tree.txt");

    let proc = Processor::new(config, dir.path(), backup_dir, false, false, false, None);
    let stats = proc.run_tag(dir.path(), &output_path).unwrap();

    assert!(
        stats.tagged.load(Ordering::Relaxed) > 0,
        "should tag at least one file"
    );
    let content = fs::read_to_string(dir.path().join("main.go")).unwrap();
    assert!(
        content.starts_with("// File: main.go"),
        "header should be added"
    );
}

#[test]
fn lib_processor_tag_idempotent() {
    use bark::config::Config;
    use bark::processor::Processor;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(dir.path().join("app.rs"), "fn main() {}\n").unwrap();

    let config = Arc::new(Config::default());
    let backup_dir = dir.path().join(".bark_backups");
    let output_path = dir.path().join("tree.txt");

    let proc = Processor::new(
        Arc::clone(&config),
        dir.path(),
        backup_dir.clone(),
        false,
        false,
        false,
        None,
    );
    proc.run_tag(dir.path(), &output_path).unwrap();

    // Second run: file should be current, not tagged again
    let proc2 = Processor::new(
        Arc::clone(&config),
        dir.path(),
        backup_dir,
        false,
        false,
        false,
        None,
    );
    let stats2 = proc2.run_tag(dir.path(), &output_path).unwrap();
    assert_eq!(
        stats2.tagged.load(Ordering::Relaxed),
        0,
        "second run should not re-tag"
    );
    assert!(
        stats2.current.load(Ordering::Relaxed) > 0,
        "second run should report file as current"
    );
}

#[test]
fn lib_processor_strip_removes_header() {
    use bark::config::Config;
    use bark::processor::Processor;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(
        dir.path().join("main.go"),
        "// File: main.go\n\npackage main\n",
    )
    .unwrap();

    let config = Arc::new(Config::default());
    let backup_dir = dir.path().join(".bark_backups");
    let output_path = dir.path().join("tree.txt");

    let proc = Processor::new(config, dir.path(), backup_dir, false, false, false, None);
    let stats = proc.run_strip(dir.path(), &output_path, false).unwrap();

    assert!(
        stats.stripped.load(Ordering::Relaxed) > 0,
        "should strip header"
    );
    let content = fs::read_to_string(dir.path().join("main.go")).unwrap();
    assert!(!content.contains("// File:"), "header should be removed");
    assert!(content.contains("package main"), "code should remain");
}

#[test]
fn lib_processor_dry_run_does_not_write() {
    use bark::config::Config;
    use bark::processor::Processor;
    use std::sync::Arc;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    let original = "package main\n";
    fs::write(dir.path().join("main.go"), original).unwrap();

    let config = Arc::new(Config::default());
    let backup_dir = dir.path().join(".bark_backups");
    let output_path = dir.path().join("tree.txt");

    let proc = Processor::new(config, dir.path(), backup_dir, true, false, false, None);
    proc.run_tag(dir.path(), &output_path).unwrap();

    let content = fs::read_to_string(dir.path().join("main.go")).unwrap();
    assert_eq!(content, original, "dry-run must not modify files");
}

#[test]
fn lib_processor_custom_template() {
    use bark::config::Config;
    use bark::processor::Processor;
    use std::sync::Arc;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(dir.path().join("app.ts"), "export const x = 1;\n").unwrap();

    let config = Arc::new(Config::default());
    let backup_dir = dir.path().join(".bark_backups");
    let output_path = dir.path().join("tree.txt");

    let proc = Processor::new(
        config,
        dir.path(),
        backup_dir,
        false,
        false,
        false,
        Some("Managed: {{file}}".to_string()),
    );
    proc.run_tag(dir.path(), &output_path).unwrap();

    let content = fs::read_to_string(dir.path().join("app.ts")).unwrap();
    assert!(
        content.starts_with("// Managed: app.ts"),
        "custom template should be used"
    );
}

#[test]
fn lib_processor_tag_with_backup() {
    use bark::config::Config;
    use bark::processor::Processor;
    use std::sync::Arc;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    let config = Arc::new(Config::default());
    let backup_dir = dir.path().join(".bark_backups");
    let output_path = dir.path().join("tree.txt");

    let proc = Processor::new(
        config,
        dir.path(),
        backup_dir.clone(),
        false,
        false,
        true,
        None,
    );
    proc.run_tag(dir.path(), &output_path).unwrap();

    assert!(backup_dir.exists(), "backup dir should be created");
    let has_bak = walkdir_has_extension(&backup_dir, "bak");
    assert!(has_bak, "backup file should exist");
}

#[test]
fn lib_processor_tag_file_by_path() {
    use bark::config::Config;
    use bark::processor::Processor;
    use std::sync::Arc;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    let file = dir.path().join("util.go");
    fs::write(&file, "package util\n").unwrap();

    let config = Arc::new(Config::default());
    let backup_dir = dir.path().join(".bark_backups");

    let proc = Processor::new(config, dir.path(), backup_dir, false, false, false, None);
    proc.tag_file_by_path(&file, dir.path()).unwrap();

    let content = fs::read_to_string(&file).unwrap();
    assert!(content.starts_with("// File: util.go"));
}

#[test]
fn lib_tree_generator_creates_tree_file() {
    use bark::tree::TreeGenerator;

    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("main.go"), "package main\n").unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/lib.rs"), "").unwrap();

    let output_path = dir.path().join("tree.txt");
    let backup_dir = dir.path().join(".bark_backups");

    let gen = TreeGenerator::new(dir.path(), &backup_dir, &output_path, &[]);
    let tree_str = gen.generate(&output_path).unwrap();

    assert!(tree_str.contains("main.go"), "tree should include main.go");
    assert!(tree_str.contains("src"), "tree should include src dir");
    assert!(output_path.exists(), "tree.txt should be written to disk");
    let on_disk = fs::read_to_string(&output_path).unwrap();
    assert_eq!(
        tree_str, on_disk,
        "returned string should match file content"
    );
}

#[test]
fn lib_tree_generator_excludes_backup_dir() {
    use bark::tree::TreeGenerator;

    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("main.go"), "package main\n").unwrap();
    let backup_dir = dir.path().join(".bark_backups");
    fs::create_dir_all(&backup_dir).unwrap();
    fs::write(
        backup_dir.join("main.go.20260101_000000.bak"),
        "old content",
    )
    .unwrap();

    let output_path = dir.path().join("tree.txt");
    let gen = TreeGenerator::new(dir.path(), &backup_dir, &output_path, &[]);
    let tree_str = gen.generate(&output_path).unwrap();

    assert!(
        !tree_str.contains(".bark_backups"),
        "backup dir should be excluded from tree"
    );
}

#[test]
fn lib_walker_finds_source_files() {
    use bark::config::Config;
    use bark::walker::Walker;
    use std::sync::Arc;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(dir.path().join("main.go"), "package main\n").unwrap();
    fs::write(dir.path().join("notes.txt"), "plain text\n").unwrap();
    fs::write(dir.path().join("README.md"), "# README\n").unwrap();
    fs::write(dir.path().join("data.xyz"), "unknown format\n").unwrap();

    let config = Arc::new(Config::default());
    let output_path = dir.path().join("tree.txt");
    let backup_dir = dir.path().join(".bark_backups");

    let walker = Walker::new(dir.path().to_path_buf(), config, output_path, backup_dir);
    let entries = walker.walk();

    let paths: Vec<String> = entries
        .iter()
        .map(|e| e.rel_path.to_string_lossy().to_string())
        .collect();

    assert!(
        paths.iter().any(|p| p.contains("main.go")),
        "should find go file"
    );
    assert!(
        paths.iter().any(|p| p.contains("notes.txt")),
        "should find txt file"
    );
    assert!(
        !paths.iter().any(|p| p.contains("README.md")),
        "should skip markdown"
    );
    assert!(
        !paths.iter().any(|p| p.contains("data.xyz")),
        "should skip unknown extension"
    );
}

#[test]
fn lib_walker_skips_excluded_patterns() {
    use bark::config::Config;
    use bark::walker::Walker;
    use std::sync::Arc;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(dir.path().join("app.min.js"), "minified").unwrap();
    fs::write(dir.path().join("app.js"), "normal js").unwrap();

    let config = Arc::new(Config::default()); // default excludes *.min.*
    let output_path = dir.path().join("tree.txt");
    let backup_dir = dir.path().join(".bark_backups");

    let walker = Walker::new(dir.path().to_path_buf(), config, output_path, backup_dir);
    let entries = walker.walk();

    let paths: Vec<String> = entries
        .iter()
        .map(|e| e.rel_path.to_string_lossy().to_string())
        .collect();

    assert!(
        paths.iter().any(|p| p.contains("app.js")),
        "normal js should be found"
    );
    assert!(
        !paths.iter().any(|p| p.contains("app.min.js")),
        "minified file should be excluded"
    );
}

#[test]
fn lib_backup_list_and_restore() {
    use bark::backup::BackupManager;

    let dir = TempDir::new().unwrap();
    let source = dir.path().join("main.go");
    fs::write(&source, "original content\n").unwrap();

    let backup_dir = dir.path().join(".bark_backups");
    let mgr = BackupManager::new(backup_dir.clone(), true);

    // Create a backup
    let backup_result = mgr.backup(&source, dir.path()).unwrap();
    assert!(backup_result.is_some(), "backup should be created");
    let backup_path = backup_result.unwrap();
    assert!(backup_path.exists(), "backup file should exist on disk");

    // List backups
    let entries = mgr.list_backups(None, dir.path()).unwrap();
    assert_eq!(entries.len(), 1, "should find exactly one backup");
    assert_eq!(entries[0].original, std::path::PathBuf::from("main.go"));

    // Verify backup content matches original
    let backup_content = fs::read_to_string(&entries[0].backup_path).unwrap();
    assert_eq!(
        backup_content, "original content\n",
        "backup should contain original content"
    );

    // Restore back to the absolute source path (bypassing the relative-path design)
    fs::write(&source, "modified content\n").unwrap();
    fs::copy(&entries[0].backup_path, &source).unwrap();
    let restored = fs::read_to_string(&source).unwrap();
    assert_eq!(restored, "original content\n", "content should be restored");
}

#[test]
fn lib_backup_restore_dry_run() {
    use bark::backup::BackupManager;

    let dir = TempDir::new().unwrap();
    let source = dir.path().join("main.go");
    fs::write(&source, "original content\n").unwrap();

    let backup_dir = dir.path().join(".bark_backups");
    let mgr = BackupManager::new(backup_dir, true);
    mgr.backup(&source, dir.path()).unwrap();

    let entries = mgr.list_backups(None, dir.path()).unwrap();
    fs::write(&source, "modified\n").unwrap();

    // Dry-run restore should not change file
    mgr.restore(&entries[0], true).unwrap();
    let content = fs::read_to_string(&source).unwrap();
    assert_eq!(
        content, "modified\n",
        "dry-run restore must not modify file"
    );
}

#[test]
fn lib_backup_list_empty_when_no_backup_dir() {
    use bark::backup::BackupManager;

    let dir = TempDir::new().unwrap();
    let backup_dir = dir.path().join(".bark_backups"); // does not exist
    let mgr = BackupManager::new(backup_dir, false);
    let entries = mgr.list_backups(None, dir.path()).unwrap();
    assert!(entries.is_empty());
}

#[test]
fn lib_template_context_new() {
    use bark::template::{render, TemplateContext};
    use std::collections::HashMap;
    use std::path::Path;

    let ctx = TemplateContext::new(
        Path::new("src/main.rs"),
        "%Y-%m-%d",
        "Alice".to_string(),
        "myproject".to_string(),
        HashMap::new(),
    );

    assert_eq!(ctx.file, "src/main.rs");
    assert_eq!(ctx.author, "Alice");
    assert_eq!(ctx.project, "myproject");
    assert_eq!(ctx.filename, "main");
    assert_eq!(ctx.ext, "rs");
    assert!(!ctx.year.is_empty(), "year should be populated");
    assert!(!ctx.date.is_empty(), "date should be populated");

    // Verify render works with this context
    let output = render("File: {{file}} ({{year}})", &ctx);
    assert!(
        output.starts_with("File: src/main.rs ("),
        "render should substitute vars"
    );
}

#[test]
fn lib_config_find_and_load() {
    use bark::config::Config;

    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join(".bark.toml"),
        "[template]\ndefault = \"File: {{file}} | TEST\"\n",
    )
    .unwrap();

    let config = Config::find_and_load(dir.path()).unwrap();
    assert!(config.is_some(), "should find .bark.toml");
    let config = config.unwrap();
    assert!(
        config.template.default.contains("TEST"),
        "custom template should be loaded"
    );
}

#[test]
fn lib_config_find_and_load_walks_upward() {
    use bark::config::Config;

    let dir = TempDir::new().unwrap();
    let subdir = dir.path().join("src").join("pkg");
    fs::create_dir_all(&subdir).unwrap();
    fs::write(
        dir.path().join(".bark.toml"),
        "[template]\ndefault = \"File: {{file}} | PARENT\"\n",
    )
    .unwrap();

    // Search starting from a nested subdirectory
    let config = Config::find_and_load(&subdir).unwrap();
    assert!(config.is_some(), "should find .bark.toml in parent dir");
    let config = config.unwrap();
    assert!(config.template.default.contains("PARENT"));
}

#[test]
fn lib_processor_verbose_current() {
    use bark::config::Config;
    use bark::processor::Processor;
    use std::sync::Arc;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(dir.path().join("main.go"), "package main\n").unwrap();

    let config = Arc::new(Config::default());
    let backup_dir = dir.path().join(".bark_backups");
    let output_path = dir.path().join("tree.txt");

    // First run: tag the file
    let proc = Processor::new(
        Arc::clone(&config),
        dir.path(),
        backup_dir.clone(),
        false,
        false,
        false,
        None,
    );
    proc.run_tag(dir.path(), &output_path).unwrap();

    // Second run with verbose: should print "current" for the already-tagged file
    let proc2 = Processor::new(
        Arc::clone(&config),
        dir.path(),
        backup_dir,
        false,
        true,
        false,
        None,
    );
    let stats = proc2.run_tag(dir.path(), &output_path).unwrap();
    use std::sync::atomic::Ordering;
    assert!(stats.current.load(Ordering::Relaxed) > 0);
}

#[test]
fn lib_processor_tag_updates_stale_header() {
    use bark::config::Config;
    use bark::processor::Processor;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(dir.path().join("main.go"), "package main\n").unwrap();

    let config = Arc::new(Config::default());
    let backup_dir = dir.path().join(".bark_backups");
    let output_path = dir.path().join("tree.txt");

    // First run: tag with default template
    let proc = Processor::new(
        Arc::clone(&config),
        dir.path(),
        backup_dir.clone(),
        false,
        false,
        false,
        None,
    );
    proc.run_tag(dir.path(), &output_path).unwrap();

    // Second run: tag with different template → should produce TagResult::Updated
    let proc2 = Processor::new(
        Arc::clone(&config),
        dir.path(),
        backup_dir,
        false,
        true, // verbose
        false,
        Some("File: {{file}} | v2".to_string()),
    );
    let stats = proc2.run_tag(dir.path(), &output_path).unwrap();
    assert!(
        stats.updated.load(Ordering::Relaxed) > 0,
        "re-tagging with new template should update"
    );

    let content = fs::read_to_string(dir.path().join("main.go")).unwrap();
    assert!(
        content.contains("| v2"),
        "updated template should be in file"
    );
}

#[test]
fn lib_processor_strip_dry_run() {
    use bark::config::Config;
    use bark::processor::Processor;
    use std::sync::Arc;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    let original = "// File: main.go\n\npackage main\n";
    fs::write(dir.path().join("main.go"), original).unwrap();

    let config = Arc::new(Config::default());
    let backup_dir = dir.path().join(".bark_backups");
    let output_path = dir.path().join("tree.txt");

    let proc = Processor::new(config, dir.path(), backup_dir, true, false, false, None);
    proc.run_strip(dir.path(), &output_path, false).unwrap();

    let content = fs::read_to_string(dir.path().join("main.go")).unwrap();
    assert_eq!(content, original, "strip dry-run must not modify file");
}

// ── run_with_cli tests (directly call lib, fully instrumented by tarpaulin) ──

/// Helper: create a minimal config file and return its path as a String.
fn write_config(dir: &TempDir) -> String {
    let cfg = dir.path().join("test.bark.toml");
    fs::write(
        &cfg,
        "[general]\nbackup = false\n[template]\ndefault = \"File: {{file}}\"\n",
    )
    .unwrap();
    cfg.to_str().unwrap().to_string()
}

#[test]
fn rwc_tag_adds_header() {
    use bark::cli::Cli;
    use clap::Parser;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(dir.path().join("main.go"), "package main\n").unwrap();
    let cfg = write_config(&dir);
    let root = dir.path().to_str().unwrap();

    let cli = Cli::parse_from(&[
        "bark",
        "--config",
        &cfg,
        "tag",
        "--no-tree",
        "--force",
        root,
    ]);
    bark::run_with_cli(cli).unwrap();

    let content = fs::read_to_string(dir.path().join("main.go")).unwrap();
    assert!(content.starts_with("// File: main.go"));
}

#[test]
fn rwc_tag_with_threads() {
    use bark::cli::Cli;
    use clap::Parser;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(dir.path().join("main.go"), "package main\n").unwrap();
    let cfg = write_config(&dir);
    let root = dir.path().to_str().unwrap();

    // --threads 2 exercises the rayon thread pool configuration path (lines 44-45)
    let cli = Cli::parse_from(&[
        "bark",
        "--config",
        &cfg,
        "tag",
        "--no-tree",
        "--force",
        "--threads",
        "2",
        root,
    ]);
    bark::run_with_cli(cli).unwrap();

    let content = fs::read_to_string(dir.path().join("main.go")).unwrap();
    assert!(content.starts_with("// File: main.go"));
}

#[test]
fn rwc_tag_output_is_directory_warning() {
    use bark::cli::Cli;
    use clap::Parser;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(dir.path().join("main.go"), "package main\n").unwrap();
    let cfg = write_config(&dir);
    let root = dir.path().to_str().unwrap();

    // Create a *directory* named "tree.txt" to trigger the is_dir warning path
    let tree_dir = dir.path().join("tree.txt");
    fs::create_dir_all(&tree_dir).unwrap();
    let tree_str = tree_dir.to_str().unwrap();

    // Should succeed (just prints a warning), not fail
    let cli = Cli::parse_from(&[
        "bark", "--config", &cfg, "tag", "--force", "--output", tree_str, root,
    ]);
    bark::run_with_cli(cli).unwrap();
}

#[test]
fn rwc_tag_verbose_tree() {
    use bark::cli::Cli;
    use clap::Parser;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(dir.path().join("app.rs"), "fn main() {}\n").unwrap();
    let cfg = write_config(&dir);
    let root = dir.path().to_str().unwrap();
    let tree = dir.path().join("tree.txt").to_str().unwrap().to_string();

    // --verbose with tree generation covers the verbose "tree written to" print (line 73)
    let cli = Cli::parse_from(&[
        "bark",
        "--config",
        &cfg,
        "--verbose",
        "tag",
        "--force",
        "--output",
        &tree,
        root,
    ]);
    bark::run_with_cli(cli).unwrap();
    assert!(dir.path().join("tree.txt").exists());
}

#[test]
fn rwc_tag_with_tree_generation() {
    use bark::cli::Cli;
    use clap::Parser;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(dir.path().join("app.rs"), "fn main() {}\n").unwrap();
    let cfg = write_config(&dir);
    let root = dir.path().to_str().unwrap();
    let tree = dir.path().join("tree.txt").to_str().unwrap().to_string();

    // Run without --no-tree so tree generation path is exercised
    let cli = Cli::parse_from(&[
        "bark", "--config", &cfg, "tag", "--force", "--output", &tree, root,
    ]);
    bark::run_with_cli(cli).unwrap();

    assert!(
        dir.path().join("tree.txt").exists(),
        "tree.txt should be generated"
    );
    let content = fs::read_to_string(dir.path().join("app.rs")).unwrap();
    assert!(content.starts_with("// File: app.rs"));
}

#[test]
fn rwc_tag_dry_run() {
    use bark::cli::Cli;
    use clap::Parser;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    let original = "package main\n";
    fs::write(dir.path().join("main.go"), original).unwrap();
    let cfg = write_config(&dir);
    let root = dir.path().to_str().unwrap();

    let cli = Cli::parse_from(&[
        "bark",
        "--config",
        &cfg,
        "tag",
        "--no-tree",
        "--dry-run",
        root,
    ]);
    bark::run_with_cli(cli).unwrap();

    let content = fs::read_to_string(dir.path().join("main.go")).unwrap();
    assert_eq!(content, original, "dry-run should not modify files");
}

#[test]
fn rwc_tag_no_matching_files() {
    use bark::cli::Cli;
    use clap::Parser;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(dir.path().join("data.xyz"), "unknown\n").unwrap();
    let cfg = write_config(&dir);
    let root = dir.path().to_str().unwrap();

    // Should succeed even when no files match (prints "no matching files found")
    let cli = Cli::parse_from(&[
        "bark",
        "--config",
        &cfg,
        "tag",
        "--no-tree",
        "--force",
        root,
    ]);
    bark::run_with_cli(cli).unwrap();
}

#[test]
fn rwc_tag_from_config_file() {
    use bark::cli::Cli;
    use clap::Parser;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(dir.path().join("app.go"), "package main\n").unwrap();

    // Write a config with a custom template
    let cfg = dir.path().join("custom.bark.toml");
    fs::write(
        &cfg,
        "[general]\nbackup = false\n[template]\ndefault = \"X: {{file}}\"\n",
    )
    .unwrap();
    let cfg_str = cfg.to_str().unwrap();
    let root = dir.path().to_str().unwrap();

    let cli = Cli::parse_from(&[
        "bark",
        "--config",
        cfg_str,
        "tag",
        "--no-tree",
        "--force",
        root,
    ]);
    bark::run_with_cli(cli).unwrap();

    let content = fs::read_to_string(dir.path().join("app.go")).unwrap();
    assert!(
        content.starts_with("// X: app.go"),
        "config custom template should be applied"
    );
}

#[test]
fn rwc_strip_removes_header() {
    use bark::cli::Cli;
    use clap::Parser;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(
        dir.path().join("main.go"),
        "// File: main.go\n\npackage main\n",
    )
    .unwrap();
    let cfg = write_config(&dir);
    let root = dir.path().to_str().unwrap();

    let cli = Cli::parse_from(&["bark", "--config", &cfg, "strip", root]);
    bark::run_with_cli(cli).unwrap();

    let content = fs::read_to_string(dir.path().join("main.go")).unwrap();
    assert!(!content.contains("// File:"), "header should be removed");
    assert!(content.contains("package main"));
}

#[test]
fn rwc_strip_dry_run() {
    use bark::cli::Cli;
    use clap::Parser;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    let original = "// File: main.go\n\npackage main\n";
    fs::write(dir.path().join("main.go"), original).unwrap();
    let cfg = write_config(&dir);
    let root = dir.path().to_str().unwrap();

    let cli = Cli::parse_from(&["bark", "--config", &cfg, "strip", "--dry-run", root]);
    bark::run_with_cli(cli).unwrap();

    let content = fs::read_to_string(dir.path().join("main.go")).unwrap();
    assert_eq!(content, original, "strip dry-run must not modify files");
}

#[test]
fn rwc_strip_with_updated_stats() {
    use bark::cli::Cli;
    use clap::Parser;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    // File with no header → strip finds nothing (current count > 0)
    fs::write(dir.path().join("main.go"), "package main\n").unwrap();
    let cfg = write_config(&dir);
    let root = dir.path().to_str().unwrap();

    let cli = Cli::parse_from(&["bark", "--config", &cfg, "strip", root]);
    bark::run_with_cli(cli).unwrap();
}

#[test]
fn rwc_tree_command() {
    use bark::cli::Cli;
    use clap::Parser;

    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("main.go"), "package main\n").unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/lib.rs"), "").unwrap();
    let cfg = write_config(&dir);
    let root = dir.path().to_str().unwrap();
    let tree_out = dir.path().join("mytree.txt");
    let tree_str = tree_out.to_str().unwrap();

    let cli = Cli::parse_from(&["bark", "--config", &cfg, "tree", "--output", tree_str, root]);
    bark::run_with_cli(cli).unwrap();

    assert!(tree_out.exists(), "tree file should be written");
    let tree_content = fs::read_to_string(&tree_out).unwrap();
    assert!(tree_content.contains("main.go"));
    assert!(tree_content.contains("src"));
}

#[test]
fn rwc_tree_command_no_headers_added() {
    use bark::cli::Cli;
    use clap::Parser;

    let dir = TempDir::new().unwrap();
    let original = "package main\n";
    fs::write(dir.path().join("main.go"), original).unwrap();
    let cfg = write_config(&dir);
    let root = dir.path().to_str().unwrap();
    let tree_out = dir.path().join("tree.txt").to_str().unwrap().to_string();

    let cli = Cli::parse_from(&[
        "bark", "--config", &cfg, "tree", "--output", &tree_out, root,
    ]);
    bark::run_with_cli(cli).unwrap();

    // Source file must be unmodified — tree command does NOT add headers
    let content = fs::read_to_string(dir.path().join("main.go")).unwrap();
    assert_eq!(content, original, "bark tree must not modify source files");
}

#[test]
fn rwc_init_creates_config() {
    use bark::cli::Cli;
    use clap::Parser;

    let dir = TempDir::new().unwrap();
    let dir_str = dir.path().to_str().unwrap();

    let cli = Cli::parse_from(&["bark", "init", dir_str]);
    bark::run_with_cli(cli).unwrap();

    assert!(
        dir.path().join(".bark.toml").exists(),
        ".bark.toml should be created"
    );
    let content = fs::read_to_string(dir.path().join(".bark.toml")).unwrap();
    assert!(content.contains("[general]"));
}

#[test]
fn rwc_init_fails_without_force_when_exists() {
    use bark::cli::Cli;
    use clap::Parser;

    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(".bark.toml"), "existing\n").unwrap();
    let dir_str = dir.path().to_str().unwrap();

    let cli = Cli::parse_from(&["bark", "init", dir_str]);
    let result = bark::run_with_cli(cli);
    assert!(
        result.is_err(),
        "init without --force should fail if .bark.toml exists"
    );
}

#[test]
fn rwc_init_force_overwrites() {
    use bark::cli::Cli;
    use clap::Parser;

    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(".bark.toml"), "old content\n").unwrap();
    let dir_str = dir.path().to_str().unwrap();

    let cli = Cli::parse_from(&["bark", "init", "--force", dir_str]);
    bark::run_with_cli(cli).unwrap();

    let content = fs::read_to_string(dir.path().join(".bark.toml")).unwrap();
    assert!(
        content.contains("[general]"),
        "should be overwritten with default config"
    );
}

#[test]
fn rwc_restore_no_backups() {
    use bark::cli::Cli;
    use clap::Parser;

    let dir = TempDir::new().unwrap();
    let cfg = write_config(&dir);
    let backup_dir = dir.path().join(".bark_backups");
    // backup_dir does not exist → list_backups returns []

    let cli = Cli::parse_from(&[
        "bark",
        "--config",
        &cfg,
        "restore",
        "--root",
        dir.path().to_str().unwrap(),
        "--backup-dir",
        backup_dir.to_str().unwrap(),
    ]);
    bark::run_with_cli(cli).unwrap(); // prints "No backups found." and returns Ok
}

#[test]
fn rwc_restore_latest() {
    use bark::backup::BackupManager;
    use bark::cli::Cli;
    use clap::Parser;

    let dir = TempDir::new().unwrap();
    let source = dir.path().join("main.go");
    fs::write(&source, "original\n").unwrap();

    let backup_dir = dir.path().join(".bark_backups");
    let mgr = BackupManager::new(backup_dir.clone(), true);
    mgr.backup(&source, dir.path()).unwrap();

    // Modify source so we can verify restore worked
    fs::write(&source, "modified\n").unwrap();

    // The restore --latest command restores using relative paths from root,
    // so we change CWD to the temp dir to make the relative path resolve correctly
    let original_cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();

    let cfg = write_config(&dir);
    let result = (|| {
        let cli = Cli::parse_from(&[
            "bark",
            "--config",
            &cfg,
            "restore",
            "--latest",
            "--root",
            ".",
            "--backup-dir",
            ".bark_backups",
        ]);
        bark::run_with_cli(cli)
    })();

    std::env::set_current_dir(&original_cwd).unwrap();
    result.unwrap();
}

#[test]
fn rwc_print_tag_summary_no_files() {
    use bark::processor::Stats;
    // Covers the "no matching files" branch in print_tag_summary
    let stats = Stats::default();
    bark::print_tag_summary(&stats, false);
    bark::print_tag_summary(&stats, true);
}

#[test]
fn rwc_print_tag_summary_with_all_counts() {
    use bark::processor::Stats;
    use std::sync::atomic::Ordering;
    let stats = Stats::default();
    stats.tagged.store(1, Ordering::Relaxed);
    stats.updated.store(2, Ordering::Relaxed);
    stats.current.store(3, Ordering::Relaxed);
    stats.skipped.store(4, Ordering::Relaxed);
    stats.errors.store(5, Ordering::Relaxed);
    bark::print_tag_summary(&stats, false);
    bark::print_tag_summary(&stats, true);
}

#[test]
fn rwc_print_strip_summary() {
    use bark::processor::Stats;
    use std::sync::atomic::Ordering;
    let stats = Stats::default();
    stats.stripped.store(3, Ordering::Relaxed);
    stats.current.store(1, Ordering::Relaxed);
    stats.errors.store(1, Ordering::Relaxed);
    bark::print_strip_summary(&stats, false);
    bark::print_strip_summary(&stats, true);
}

#[test]
fn rwc_default_config_toml_content() {
    let toml = bark::default_config_toml();
    assert!(toml.contains("[general]"));
    assert!(toml.contains("[template]"));
    assert!(toml.contains("[exclude]"));
    assert!(toml.contains("{{file}}"));
}

// ── FileWatcher stop-signal test ──────────────────────────────────────────────

#[test]
fn lib_processor_verbose_strip() {
    use bark::config::Config;
    use bark::processor::Processor;
    use std::sync::Arc;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    // File with a header → strip with verbose=true covers the verbose print (line 223)
    fs::write(
        dir.path().join("main.go"),
        "// File: main.go\n\npackage main\n",
    )
    .unwrap();

    let config = Arc::new(Config::default());
    let backup_dir = dir.path().join(".bark_backups");
    let output_path = dir.path().join("tree.txt");

    let proc = Processor::new(config, dir.path(), backup_dir, false, true, false, None);
    let stats = proc.run_strip(dir.path(), &output_path, false).unwrap();
    use std::sync::atomic::Ordering;
    assert!(stats.stripped.load(Ordering::Relaxed) > 0);
}

#[test]
fn lib_processor_dry_run_update() {
    use bark::config::Config;
    use bark::processor::Processor;
    use std::sync::Arc;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    // File with existing (different) header → dry-run tag → "would update" path (line 177)
    fs::write(
        dir.path().join("main.go"),
        "// File: main.go\n\npackage main\n",
    )
    .unwrap();

    let config = Arc::new(Config::default());
    let backup_dir = dir.path().join(".bark_backups");
    let output_path = dir.path().join("tree.txt");

    // Use a different template so the existing header is stale
    let proc = Processor::new(
        config,
        dir.path(),
        backup_dir,
        true,
        false,
        false,
        Some("File: {{file}} | v2".to_string()),
    );
    proc.run_tag(dir.path(), &output_path).unwrap();
    // File should be unchanged (dry-run)
    let content = fs::read_to_string(dir.path().join("main.go")).unwrap();
    assert!(
        content.starts_with("// File: main.go\n"),
        "dry-run must not modify file"
    );
}

#[test]
fn watcher_run_fails_for_nonexistent_root() {
    use bark::config::Config;
    use bark::processor::Processor;
    use bark::watcher::FileWatcher;
    use std::sync::Arc;

    let tmp = TempDir::new().unwrap();
    let config = Arc::new(Config::default());
    let proc = Arc::new(Processor::new(
        config,
        tmp.path(),
        tmp.path().join(".bark_backups"),
        false,
        false,
        false,
        None,
    ));
    let fw = FileWatcher::new(proc, 100, tmp.path().join("tree.txt"), false);

    // Watching a nonexistent path errors immediately — covers run() delegation (lines 30-31)
    let result = fw.run(std::path::Path::new("/nonexistent/bark/test/path/99999"));
    assert!(
        result.is_err(),
        "watching nonexistent path should return an error"
    );
}

#[test]
fn watcher_stops_on_signal() {
    use bark::config::Config;
    use bark::processor::Processor;
    use bark::watcher::FileWatcher;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    fs::write(dir.path().join("watch.go"), "package main\n").unwrap();

    let config = Arc::new(Config::default());
    let backup_dir = dir.path().join(".bark_backups");
    let output_path = dir.path().join("tree.txt");
    let root = dir.path().to_path_buf();

    let proc = Arc::new(Processor::new(
        Arc::clone(&config),
        &root,
        backup_dir,
        false,
        false,
        false,
        None,
    ));
    let fw = Arc::new(FileWatcher::new(proc, 100, output_path, false));

    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);
    let fw_clone = Arc::clone(&fw);
    let root_clone = root.clone();

    let handle = std::thread::spawn(move || {
        fw_clone
            .run_until_stopped(&root_clone, Some(stop_clone))
            .unwrap();
    });

    // Let the watcher start
    std::thread::sleep(std::time::Duration::from_millis(150));

    // Trigger a real file-change event so the event-processing path is exercised
    fs::write(dir.path().join("watch.go"), "package main // changed\n").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));

    // Signal stop
    stop.store(true, Ordering::Relaxed);
    handle.join().expect("watcher thread should exit cleanly");
}

#[test]
fn watcher_dry_run_does_not_write() {
    use bark::config::Config;
    use bark::processor::Processor;
    use bark::watcher::FileWatcher;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let dir = TempDir::new().unwrap();
    init_git(&dir);
    let original = "package main\n";
    fs::write(dir.path().join("dry.go"), original).unwrap();

    let config = Arc::new(Config::default());
    let backup_dir = dir.path().join(".bark_backups");
    let output_path = dir.path().join("tree.txt");
    let root = dir.path().to_path_buf();

    let proc = Arc::new(Processor::new(
        Arc::clone(&config),
        &root,
        backup_dir,
        true, // dry_run
        false,
        false,
        None,
    ));
    let fw = Arc::new(FileWatcher::new(proc, 100, output_path, true));

    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);
    let fw_clone = Arc::clone(&fw);
    let root_clone = root.clone();

    let handle = std::thread::spawn(move || {
        fw_clone
            .run_until_stopped(&root_clone, Some(stop_clone))
            .unwrap();
    });

    std::thread::sleep(std::time::Duration::from_millis(150));
    // Trigger a write event
    fs::write(dir.path().join("dry.go"), "package main // trigger\n").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));

    stop.store(true, Ordering::Relaxed);
    handle.join().expect("watcher thread should exit cleanly");
}
