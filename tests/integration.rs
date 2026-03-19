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
        .stdout(predicate::str::contains("1.0.0"));
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
    bark().arg("init").current_dir(dir.path()).assert().success();

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

    bark().arg("init").current_dir(dir.path()).assert().success();
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
    fs::write(dir.path().join("main.go"), "package main\n\nfunc main() {}\n").unwrap();

    bark()
        .args(["tag", "--force", "--no-tree"])
        .current_dir(dir.path())
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join("main.go")).unwrap();
    assert!(content.starts_with("// File: main.go"), "header should be on line 0");
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
    assert_eq!(after_first, after_second, "second run should not modify files");
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
        .args(["tag", "--force", "--no-tree", "--template", "File: {{file}} | Bark"])
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
    assert!(backup_dir.exists(), ".bark_backups directory should be created");

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
    assert_eq!(content, "some content\n", "unknown extension should be untouched");
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
    ).unwrap();

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

    assert!(ts.starts_with("// File: app.ts"),    "ts uses slash style");
    assert!(css.starts_with("/* File: style.css */"), "css uses css style");
    assert!(html.starts_with("<!-- File: index.html -->"), "html uses html style");
    assert!(toml_file.starts_with("# File: config.toml"), "toml uses hash style");
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
    ).unwrap();

    bark()
        .args(["tag", "--force", "--no-tree"])
        .current_dir(dir.path())
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join("main.go")).unwrap();
    assert!(content.contains("| CUSTOM"), "config template should be used");
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
