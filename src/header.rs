// File: src/header.rs
use crate::template::TemplateContext;
use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentStyle {
    Slash,  // // comment
    Hash,   // # comment
    Css,    // /* comment */
    Html,   // <!-- comment -->
}

impl CommentStyle {
    pub fn from_ext(ext: &str) -> Option<Self> {
        match ext {
            // C-family, JVM, systems, modern compiled
            "go" | "rs" | "js" | "mjs" | "cjs"
            | "ts" | "tsx" | "jsx" | "mts" | "cts"
            | "java" | "kt" | "kts" | "scala" | "groovy"
            | "cpp" | "cc" | "cxx" | "c" | "h" | "hpp" | "hxx"
            | "cs" | "swift" | "m" | "mm"
            | "dart" | "php"
            // Systems / low-level
            | "zig" | "v" | "odin" | "gleam"
            // Web / shader
            | "wgsl" | "glsl" | "hlsl"
            // Other compiled
            | "sol" | "proto" | "thrift"
            | "fs" | "fsi" | "fsx"    // F#
            | "purs" | "elm"           // functional
            => Some(Self::Slash),

            // Config / data / scripting languages (non-shell)
            "py" | "rb" | "cr" | "nim"
            | "ex" | "exs"             // Elixir
            | "jl"                     // Julia
            | "tf" | "tfvars" | "hcl"  // Terraform / HCL
            | "nix"                    // Nix
            | "graphql" | "gql"
            | "md" | "txt"
            | "toml" | "yaml" | "yml"
            => Some(Self::Hash),

            "css" | "scss" | "sass" | "less" | "styl" => Some(Self::Css),

            "html" | "htm" | "xml" | "svg"
            | "vue" | "svelte" | "astro"
            => Some(Self::Html),

            _ => None,
        }
    }

    /// Wrap a rendered template body in this comment style.
    pub fn wrap(&self, body: &str) -> String {
        match self {
            Self::Slash => format!("// {}", body),
            Self::Hash  => format!("# {}", body),
            Self::Css   => format!("/* {} */", body),
            Self::Html  => format!("<!-- {} -->", body),
        }
    }

    /// Regex that broadly matches ANY bark header for this comment style.
    /// Intentionally broad so it catches headers written with any template.
    pub fn detect_regex(&self) -> Regex {
        let pattern = match self {
            Self::Slash => r"^[[:space:]]*//[[:space:]]+\S",
            Self::Hash  => r"^[[:space:]]*#[[:space:]]+\S",
            Self::Css   => r"^[[:space:]]*/\*[[:space:]]+\S",
            Self::Html  => r"^[[:space:]]*<!--[[:space:]]+\S",
        };
        Regex::new(pattern).expect("valid regex")
    }
}

pub enum HeaderAction {
    AlreadyCurrent,
    UpdateExisting,
    AddNew,
}

/// Build the full header line for a file.
pub fn build_header(style: CommentStyle, template: &str, ctx: &TemplateContext) -> String {
    let body = crate::template::render(template, ctx);
    style.wrap(&body)
}

/// Determine what action should be taken on a file's header.
pub fn analyze(content: &str, desired_header: &str, style: CommentStyle) -> HeaderAction {
    let re = style.detect_regex();
    let candidate = candidate_line(content);
    match candidate {
        Some(line) if re.is_match(line) => {
            if line.trim() == desired_header.trim() {
                HeaderAction::AlreadyCurrent
            } else {
                HeaderAction::UpdateExisting
            }
        }
        _ => HeaderAction::AddNew,
    }
}

/// Return the line to inspect for an existing header.
/// Skips shebangs so the header candidate is always line 0 or 1.
fn candidate_line(content: &str) -> Option<&str> {
    let mut lines = content.lines();
    let first = lines.next()?;
    if first.starts_with("#!") {
        lines.next()
    } else {
        Some(first)
    }
}

/// Apply the header tag — return new file content with header added/updated.
pub fn apply_tag(content: &str, desired_header: &str, style: CommentStyle) -> String {
    let re = style.detect_regex();
    let line_ending = if content.contains("\r\n") { "\r\n" } else { "\n" };
    let trailing_nl = content.ends_with('\n') || content.ends_with("\r\n");

    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() {
        return format!("{}{}", desired_header, line_ending);
    }

    let has_shebang = lines[0].starts_with("#!");

    let mut out: Vec<String> = Vec::new();

    if has_shebang {
        out.push(lines[0].to_string());
        // Check whether line 1 is an existing header
        let header_idx = if lines.len() > 1 && re.is_match(lines[1]) { 1 } else { usize::MAX };
        let mut rest_start = if header_idx == 1 {
            if lines.get(2).map_or(false, |l| l.trim().is_empty()) { 3 } else { 2 }
        } else {
            1
        };
        out.push(desired_header.to_string());
        // Exactly one blank line after header — skip any existing leading blanks
        while rest_start < lines.len() && lines[rest_start].trim().is_empty() {
            rest_start += 1;
        }
        out.push(String::new());
        out.extend(lines[rest_start..].iter().map(|l| l.to_string()));
    } else {
        let mut rest_start = if re.is_match(lines[0]) {
            if lines.get(1).map_or(false, |l| l.trim().is_empty()) { 2 } else { 1 }
        } else {
            0
        };
        out.push(desired_header.to_string());
        // Exactly one blank line after header — skip any existing leading blanks
        while rest_start < lines.len() && lines[rest_start].trim().is_empty() {
            rest_start += 1;
        }
        out.push(String::new());
        out.extend(lines[rest_start..].iter().map(|l| l.to_string()));
    }

    let mut result = out.join(line_ending);
    if trailing_nl && !result.ends_with('\n') {
        result.push_str(line_ending);
    }
    result
}

/// Strip any bark-managed header from the file content.
pub fn strip(content: &str, style: CommentStyle) -> Option<String> {
    let re = style.detect_regex();
    let line_ending = if content.contains("\r\n") { "\r\n" } else { "\n" };
    let trailing_nl = content.ends_with('\n') || content.ends_with("\r\n");

    let lines: Vec<&str> = content.lines().collect();
    let has_shebang = lines.first().map_or(false, |l| l.starts_with("#!"));

    // Determine which line to check for header
    let check_idx = if has_shebang { 1 } else { 0 };

    // Check up to first 3 lines for a header
    let header_idx = lines
        .iter()
        .enumerate()
        .skip(check_idx)
        .take(3)
        .find(|(_, l)| re.is_match(l))
        .map(|(i, _)| i);

    let Some(idx) = header_idx else {
        return None; // no header found — nothing to do
    };

    let mut out: Vec<&str> = lines.clone();
    out.remove(idx);
    // Remove blank line that immediately follows where the header was, if any
    if idx < out.len() && out[idx].trim().is_empty() {
        out.remove(idx);
    }

    let mut result = out.join(line_ending);
    if trailing_nl && !result.ends_with('\n') {
        result.push_str(line_ending);
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn ctx(file: &str) -> TemplateContext {
        TemplateContext {
            file: file.to_string(),
            date: "2026-03-19".to_string(),
            year: "2026".to_string(),
            author: "tester".to_string(),
            project: "myproject".to_string(),
            filename: file.split('/').last().unwrap_or(file).trim_end_matches(".rs").to_string(),
            ext: "rs".to_string(),
            custom: HashMap::new(),
        }
    }

    // ── CommentStyle::from_ext ──────────────────────────────────────────────

    #[test]
    fn slash_extensions() {
        for ext in &["go", "rs", "js", "ts", "tsx", "jsx", "java", "kt", "cpp",
                     "c", "h", "cs", "swift", "dart", "zig", "sol", "proto"] {
            assert_eq!(CommentStyle::from_ext(ext), Some(CommentStyle::Slash),
                "expected Slash for .{}", ext);
        }
    }

    #[test]
    fn hash_extensions() {
        for ext in &["py", "rb", "cr", "nim", "ex", "exs", "jl", "tf", "nix",
                     "graphql", "md", "toml", "yaml", "yml"] {
            assert_eq!(CommentStyle::from_ext(ext), Some(CommentStyle::Hash),
                "expected Hash for .{}", ext);
        }
    }

    #[test]
    fn css_extensions() {
        for ext in &["css", "scss", "sass", "less", "styl"] {
            assert_eq!(CommentStyle::from_ext(ext), Some(CommentStyle::Css),
                "expected Css for .{}", ext);
        }
    }

    #[test]
    fn html_extensions() {
        for ext in &["html", "htm", "xml", "svg", "vue", "svelte", "astro"] {
            assert_eq!(CommentStyle::from_ext(ext), Some(CommentStyle::Html),
                "expected Html for .{}", ext);
        }
    }

    #[test]
    fn unknown_extension_returns_none() {
        assert_eq!(CommentStyle::from_ext("xyz"), None);
        assert_eq!(CommentStyle::from_ext(""), None);
        assert_eq!(CommentStyle::from_ext("sh"), None);  // removed intentionally
        assert_eq!(CommentStyle::from_ext("bash"), None);
    }

    // ── CommentStyle::wrap ──────────────────────────────────────────────────

    #[test]
    fn wrap_slash() {
        assert_eq!(CommentStyle::Slash.wrap("File: foo.rs"), "// File: foo.rs");
    }

    #[test]
    fn wrap_hash() {
        assert_eq!(CommentStyle::Hash.wrap("File: foo.py"), "# File: foo.py");
    }

    #[test]
    fn wrap_css() {
        assert_eq!(CommentStyle::Css.wrap("File: foo.css"), "/* File: foo.css */");
    }

    #[test]
    fn wrap_html() {
        assert_eq!(CommentStyle::Html.wrap("File: foo.html"), "<!-- File: foo.html -->");
    }

    // ── detect_regex ────────────────────────────────────────────────────────

    #[test]
    fn detect_regex_slash_matches() {
        let re = CommentStyle::Slash.detect_regex();
        assert!(re.is_match("// File: src/main.rs"));
        assert!(re.is_match("// anything here"));
        assert!(!re.is_match("# File: foo.py"));
        assert!(!re.is_match(""));
    }

    #[test]
    fn detect_regex_hash_matches() {
        let re = CommentStyle::Hash.detect_regex();
        assert!(re.is_match("# File: foo.py"));
        assert!(!re.is_match("// File: foo.rs"));
    }

    #[test]
    fn detect_regex_css_matches() {
        let re = CommentStyle::Css.detect_regex();
        assert!(re.is_match("/* File: foo.css */"));
        assert!(!re.is_match("// File: foo.rs"));
    }

    #[test]
    fn detect_regex_html_matches() {
        let re = CommentStyle::Html.detect_regex();
        assert!(re.is_match("<!-- File: foo.html -->"));
        assert!(!re.is_match("// File: foo.rs"));
    }

    // ── build_header ────────────────────────────────────────────────────────

    #[test]
    fn build_header_slash() {
        let c = ctx("src/main.rs");
        let h = build_header(CommentStyle::Slash, "File: {{file}}", &c);
        assert_eq!(h, "// File: src/main.rs");
    }

    #[test]
    fn build_header_hash() {
        let c = ctx("script.py");
        let h = build_header(CommentStyle::Hash, "File: {{file}}", &c);
        assert_eq!(h, "# File: script.py");
    }

    // ── analyze ─────────────────────────────────────────────────────────────

    #[test]
    fn analyze_add_new() {
        let content = "package main\n\nfunc main() {}\n";
        let desired = "// File: main.go";
        assert!(matches!(analyze(content, desired, CommentStyle::Slash), HeaderAction::AddNew));
    }

    #[test]
    fn analyze_already_current() {
        let content = "// File: main.go\n\npackage main\n";
        let desired = "// File: main.go";
        assert!(matches!(analyze(content, desired, CommentStyle::Slash), HeaderAction::AlreadyCurrent));
    }

    #[test]
    fn analyze_update_existing() {
        let content = "// File: old/path.go\n\npackage main\n";
        let desired = "// File: new/path.go";
        assert!(matches!(analyze(content, desired, CommentStyle::Slash), HeaderAction::UpdateExisting));
    }

    #[test]
    fn analyze_skips_shebang() {
        let content = "#!/usr/bin/env python3\n# File: script.py\n\nprint('hi')\n";
        let desired = "# File: script.py";
        assert!(matches!(analyze(content, desired, CommentStyle::Hash), HeaderAction::AlreadyCurrent));
    }

    // ── apply_tag ───────────────────────────────────────────────────────────

    #[test]
    fn apply_tag_empty_file() {
        let result = apply_tag("", "// File: foo.rs", CommentStyle::Slash);
        assert_eq!(result, "// File: foo.rs\n");
    }

    #[test]
    fn apply_tag_add_new_has_blank_line() {
        let content = "package main\n\nfunc main() {}\n";
        let result = apply_tag(content, "// File: main.go", CommentStyle::Slash);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines[0], "// File: main.go");
        assert_eq!(lines[1], "");              // blank line
        assert_eq!(lines[2], "package main");
    }

    #[test]
    fn apply_tag_update_replaces_not_duplicates() {
        let content = "// File: old.go\n\npackage main\n";
        let result = apply_tag(content, "// File: new.go", CommentStyle::Slash);
        assert!(result.starts_with("// File: new.go\n"));
        assert!(!result.contains("// File: old.go"));
    }

    #[test]
    fn apply_tag_preserves_shebang() {
        let content = "#!/usr/bin/env python3\nprint('hi')\n";
        let result = apply_tag(content, "# File: script.py", CommentStyle::Hash);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines[0], "#!/usr/bin/env python3");
        assert_eq!(lines[1], "# File: script.py");
        assert_eq!(lines[2], "");             // blank line
        assert_eq!(lines[3], "print('hi')");
    }

    #[test]
    fn apply_tag_crlf_preserved() {
        let content = "package main\r\n\r\nfunc main() {}\r\n";
        let result = apply_tag(content, "// File: main.go", CommentStyle::Slash);
        assert!(result.contains("\r\n"), "CRLF should be preserved");
        assert!(result.starts_with("// File: main.go\r\n"));
    }

    #[test]
    fn apply_tag_exactly_one_blank_line_after_header() {
        // Even if content starts with multiple blanks, we should get exactly one
        let content = "\n\n\npackage main\n";
        let result = apply_tag(content, "// File: main.go", CommentStyle::Slash);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines[0], "// File: main.go");
        assert_eq!(lines[1], "");
        assert_eq!(lines[2], "package main");
    }

    #[test]
    fn apply_tag_trailing_newline_preserved() {
        let content = "package main\n";
        let result = apply_tag(content, "// File: main.go", CommentStyle::Slash);
        assert!(result.ends_with('\n'));
    }

    // ── strip ────────────────────────────────────────────────────────────────

    #[test]
    fn strip_removes_header_and_blank() {
        let content = "// File: main.go\n\npackage main\n";
        let result = strip(content, CommentStyle::Slash);
        assert!(result.is_some());
        let stripped = result.unwrap();
        assert!(!stripped.contains("// File:"));
        assert!(stripped.starts_with("package main"));
    }

    #[test]
    fn strip_returns_none_when_no_header() {
        let content = "package main\n\nfunc main() {}\n";
        assert!(strip(content, CommentStyle::Slash).is_none());
    }

    #[test]
    fn strip_preserves_shebang() {
        let content = "#!/usr/bin/env python3\n# File: script.py\n\nprint('hi')\n";
        let result = strip(content, CommentStyle::Hash).unwrap();
        assert!(result.starts_with("#!/usr/bin/env python3"));
        assert!(!result.contains("# File:"));
        assert!(result.contains("print('hi')"));
    }

    #[test]
    fn strip_css_header() {
        let content = "/* File: styles.css */\n\nbody { margin: 0; }\n";
        let result = strip(content, CommentStyle::Css).unwrap();
        assert!(!result.contains("/* File:"));
        assert!(result.contains("body { margin: 0; }"));
    }

    #[test]
    fn strip_html_header() {
        let content = "<!-- File: index.html -->\n\n<html></html>\n";
        let result = strip(content, CommentStyle::Html).unwrap();
        assert!(!result.contains("<!-- File:"));
        assert!(result.contains("<html></html>"));
    }

    // ── shebang + existing header edge cases (covers lines 138 and 145) ──────

    #[test]
    fn apply_tag_shebang_with_existing_header_and_blank() {
        // shebang → existing header → blank line → code
        // Line 138: the `{ 3 }` branch — skip header + blank → rest_start = 3
        let content = "#!/usr/bin/env python3\n# File: old.py\n\nprint('hi')\n";
        let result = apply_tag(content, "# File: script.py", CommentStyle::Hash);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines[0], "#!/usr/bin/env python3");
        assert_eq!(lines[1], "# File: script.py");
        assert_eq!(lines[2], "");
        assert_eq!(lines[3], "print('hi')");
        // Old header must be replaced, not duplicated
        assert_eq!(result.matches("# File:").count(), 1);
    }

    #[test]
    fn apply_tag_shebang_existing_header_multiple_blanks() {
        // shebang → existing header → multiple blank lines → code
        // Line 145: rest_start += 1 in blank-dedup loop (shebang path)
        let content = "#!/usr/bin/env python3\n# File: old.py\n\n\n\nprint('hi')\n";
        let result = apply_tag(content, "# File: script.py", CommentStyle::Hash);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines[0], "#!/usr/bin/env python3");
        assert_eq!(lines[1], "# File: script.py");
        assert_eq!(lines[2], "");             // exactly one blank
        assert_eq!(lines[3], "print('hi')"); // multiple blanks collapsed
    }
}
