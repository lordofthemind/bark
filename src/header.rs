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
            "go" | "rs" | "js" | "ts" | "jsx" | "tsx" | "java" | "kt" | "kts"
            | "cpp" | "cc" | "cxx" | "c" | "h" | "hpp" | "cs" | "swift" | "php"
            | "scala" | "groovy" | "dart" => Some(Self::Slash),

            "py" | "rb" | "sh" | "bash" | "zsh" | "fish" | "pl" | "pm" | "r"
            | "md" | "txt" | "toml" | "yaml" | "yml" => Some(Self::Hash),

            "css" | "scss" | "sass" | "less" => Some(Self::Css),

            "html" | "htm" | "xml" | "svg" | "vue" | "svelte" => Some(Self::Html),

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
        let rest_start = if header_idx == 1 {
            // Skip old header and optional blank line after it
            if lines.get(2).map_or(false, |l| l.trim().is_empty()) { 3 } else { 2 }
        } else {
            1
        };
        out.push(desired_header.to_string());
        out.extend(lines[rest_start..].iter().map(|l| l.to_string()));
    } else {
        let rest_start = if re.is_match(lines[0]) {
            if lines.get(1).map_or(false, |l| l.trim().is_empty()) { 2 } else { 1 }
        } else {
            0
        };
        out.push(desired_header.to_string());
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
