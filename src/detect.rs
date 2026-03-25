// File: src/detect.rs

use std::path::Path;

#[derive(Debug, PartialEq)]
pub enum ProjectKind {
    React,
    TypeScript,
    Go,
    Rust,
    Terraform,
    Docker,
}

pub fn detect(dir: &Path) -> Vec<ProjectKind> {
    let mut kinds = Vec::new();

    if dir.join("package.json").is_file() {
        kinds.push(ProjectKind::React);
    }
    if dir.join("tsconfig.json").is_file() {
        kinds.push(ProjectKind::TypeScript);
    }
    if dir.join("go.mod").is_file() {
        kinds.push(ProjectKind::Go);
    }
    if dir.join("Cargo.toml").is_file() {
        kinds.push(ProjectKind::Rust);
    }
    if dir.join("terraform").is_dir()
        || std::fs::read_dir(dir)
            .map(|mut d| {
                d.any(|e| {
                    e.ok()
                        .and_then(|e| {
                            e.file_name()
                                .to_string_lossy()
                                .ends_with(".tf")
                                .then_some(())
                        })
                        .is_some()
                })
            })
            .unwrap_or(false)
    {
        kinds.push(ProjectKind::Terraform);
    }
    if dir.join("Dockerfile").is_file()
        || dir.join("docker-compose.yml").is_file()
        || dir.join("docker-compose.yaml").is_file()
    {
        kinds.push(ProjectKind::Docker);
    }

    kinds
}

pub fn generate_config(kinds: &[ProjectKind]) -> String {
    let mut excludes = vec!["\"*.min.*\"".to_string(), "\"*.bundle.*\"".to_string()];
    let mut header_skip_hints = Vec::<String>::new();
    let mut comments = Vec::<String>::new();

    if kinds.contains(&ProjectKind::React) || kinds.contains(&ProjectKind::TypeScript) {
        excludes.push("\"dist/**\"".to_string());
        excludes.push("\"build/**\"".to_string());
        excludes.push("\"out/**\"".to_string());
        excludes.push("\".next/**\"".to_string());
        excludes.push("\"node_modules/**\"".to_string());
        excludes.push("\"coverage/**\"".to_string());
        header_skip_hints.push("\"*.md\"".to_string());
        header_skip_hints.push("\"*.mdx\"".to_string());
        comments.push("# React / TypeScript project detected".to_string());
    }

    if kinds.contains(&ProjectKind::Go) {
        excludes.push("\"vendor/**\"".to_string());
        if !excludes.contains(&"\"dist/**\"".to_string()) {
            excludes.push("\"dist/**\"".to_string());
            excludes.push("\"build/**\"".to_string());
        }
        comments.push("# Go project detected".to_string());
    }

    if kinds.contains(&ProjectKind::Rust) {
        if !excludes.iter().any(|e| e.contains("target")) {
            excludes.push("\"target/**\"".to_string());
        }
        comments.push("# Rust project detected".to_string());
    } else {
        // Add target anyway as a sensible default
        excludes.push("\"target/**\"".to_string());
    }

    if kinds.contains(&ProjectKind::Terraform) {
        excludes.push("\".terraform/**\"".to_string());
        comments
            .push("# Terraform project detected — .tf files are supported natively".to_string());
    }

    if kinds.contains(&ProjectKind::Docker) {
        comments.push(
            "# Docker project detected — add Dockerfile support via [extensions.filenames]"
                .to_string(),
        );
    }

    if kinds.is_empty() {
        excludes.push("\"dist/**\"".to_string());
        excludes.push("\"build/**\"".to_string());
        excludes.push("\"node_modules/**\"".to_string());
        excludes.push("\"vendor/**\"".to_string());
        excludes.push("\"target/**\"".to_string());
        excludes.push("\"coverage/**\"".to_string());
    }

    let patterns_str = excludes
        .iter()
        .map(|e| format!("    {}", e))
        .collect::<Vec<_>>()
        .join(",\n");
    let header_skip_str = if header_skip_hints.is_empty() {
        "[]".to_string()
    } else {
        format!("[{}]", header_skip_hints.join(", "))
    };
    let comments_str = if comments.is_empty() {
        String::new()
    } else {
        format!("{}\n", comments.join("\n"))
    };

    format!(
        r#"{comments_str}[general]
output      = "bark.txt"        # tree output filename
backup_dir  = ".barks"          # where backups are stored
max_file_size = 1048576         # skip files larger than this (bytes)
backup      = true              # create backups before modifying files

[template]
# Available variables: {{{{file}}}}, {{{{date}}}}, {{{{year}}}}, {{{{author}}}}, {{{{project}}}}, {{{{filename}}}}, {{{{ext}}}}
default     = "File: {{{{file}}}}"
date_format = "%Y-%m-%d"

[template.overrides]
# rs = "File: {{{{file}}}} | Author: {{{{author}}}} | {{{{date}}}}"

[template.variables]
# author  = "Your Name"
# project = "my-project"

[exclude]
patterns = [
{patterns_str},
]
header_skip = {header_skip_str}

[extensions]
custom = []
skip = []
filenames = [
    # {{ name = "Dockerfile",  style = "hash"  }},
    # {{ name = "Jenkinsfile", style = "slash" }},
]
filename_skip = []

[watch]
debounce_ms = 500
ignore      = []
"#,
        comments_str = comments_str,
        patterns_str = patterns_str,
        header_skip_str = header_skip_str,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── detect() ─────────────────────────────────────────────────────────────

    #[test]
    fn detect_empty_dir_returns_no_kinds() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(detect(tmp.path()).is_empty());
    }

    #[test]
    fn detect_rust_via_cargo_toml() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "[package]").unwrap();
        assert!(detect(tmp.path()).contains(&ProjectKind::Rust));
    }

    #[test]
    fn detect_go_via_go_mod() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("go.mod"), "module example").unwrap();
        assert!(detect(tmp.path()).contains(&ProjectKind::Go));
    }

    #[test]
    fn detect_react_via_package_json() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("package.json"), "{}").unwrap();
        assert!(detect(tmp.path()).contains(&ProjectKind::React));
    }

    #[test]
    fn detect_typescript_via_tsconfig() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("tsconfig.json"), "{}").unwrap();
        assert!(detect(tmp.path()).contains(&ProjectKind::TypeScript));
    }

    #[test]
    fn detect_terraform_via_tf_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("main.tf"), "provider {}").unwrap();
        assert!(detect(tmp.path()).contains(&ProjectKind::Terraform));
    }

    #[test]
    fn detect_terraform_via_terraform_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join("terraform")).unwrap();
        assert!(detect(tmp.path()).contains(&ProjectKind::Terraform));
    }

    #[test]
    fn detect_docker_via_dockerfile() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("Dockerfile"), "FROM ubuntu").unwrap();
        assert!(detect(tmp.path()).contains(&ProjectKind::Docker));
    }

    #[test]
    fn detect_docker_via_compose_yml() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("docker-compose.yml"), "version: '3'").unwrap();
        assert!(detect(tmp.path()).contains(&ProjectKind::Docker));
    }

    #[test]
    fn detect_docker_via_compose_yaml() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("docker-compose.yaml"), "version: '3'").unwrap();
        assert!(detect(tmp.path()).contains(&ProjectKind::Docker));
    }

    #[test]
    fn detect_multiple_kinds_at_once() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "[package]").unwrap();
        std::fs::write(tmp.path().join("go.mod"), "module example").unwrap();
        let kinds = detect(tmp.path());
        assert!(kinds.contains(&ProjectKind::Rust));
        assert!(kinds.contains(&ProjectKind::Go));
    }

    // ── generate_config() ────────────────────────────────────────────────────

    #[test]
    fn generate_config_empty_includes_all_common_excludes() {
        let s = generate_config(&[]);
        assert!(s.contains("[general]"));
        assert!(s.contains("node_modules/**"));
        assert!(s.contains("vendor/**"));
        assert!(s.contains("target/**"));
        assert!(s.contains("dist/**"));
        assert!(s.contains("coverage/**"));
    }

    #[test]
    fn generate_config_rust_adds_target_exactly_once() {
        let s = generate_config(&[ProjectKind::Rust]);
        assert!(s.contains("# Rust project detected"));
        assert_eq!(s.matches("\"target/**\"").count(), 1);
    }

    #[test]
    fn generate_config_non_rust_still_adds_target() {
        let s = generate_config(&[ProjectKind::Docker]);
        assert!(s.contains("\"target/**\""));
    }

    #[test]
    fn generate_config_go_adds_vendor() {
        let s = generate_config(&[ProjectKind::Go]);
        assert!(s.contains("\"vendor/**\""));
        assert!(s.contains("# Go project detected"));
    }

    #[test]
    fn generate_config_go_adds_dist_when_react_absent() {
        let s = generate_config(&[ProjectKind::Go]);
        assert!(s.contains("\"dist/**\""));
    }

    #[test]
    fn generate_config_react_adds_node_modules_and_header_skip() {
        let s = generate_config(&[ProjectKind::React]);
        assert!(s.contains("node_modules/**"));
        assert!(s.contains("# React / TypeScript project detected"));
        assert!(s.contains("\"*.md\""));
        assert!(s.contains("\"*.mdx\""));
    }

    #[test]
    fn generate_config_typescript_same_as_react_block() {
        let s = generate_config(&[ProjectKind::TypeScript]);
        assert!(s.contains("# React / TypeScript project detected"));
        assert!(s.contains("node_modules/**"));
    }

    #[test]
    fn generate_config_terraform_adds_dot_terraform() {
        let s = generate_config(&[ProjectKind::Terraform]);
        assert!(s.contains(".terraform/**"));
        assert!(s.contains("# Terraform project detected"));
    }

    #[test]
    fn generate_config_docker_adds_comment() {
        let s = generate_config(&[ProjectKind::Docker]);
        assert!(s.contains("# Docker project detected"));
    }

    #[test]
    fn generate_config_react_and_go_no_duplicate_dist() {
        let s = generate_config(&[ProjectKind::React, ProjectKind::Go]);
        assert_eq!(
            s.matches("\"dist/**\"").count(),
            1,
            "dist/** should not be duplicated when React is present"
        );
    }

    #[test]
    fn generate_config_empty_no_project_comments() {
        let s = generate_config(&[]);
        assert!(!s.contains("# Rust project detected"));
        assert!(!s.contains("# Go project detected"));
        assert!(!s.contains("# React"));
    }

    #[test]
    fn generate_config_output_is_valid_toml_structure() {
        let s = generate_config(&[ProjectKind::Rust, ProjectKind::Go]);
        assert!(s.contains("[general]"));
        assert!(s.contains("[template]"));
        assert!(s.contains("[exclude]"));
        assert!(s.contains("[watch]"));
    }
}
