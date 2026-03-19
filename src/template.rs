// File: src/template.rs
use chrono::Local;
use std::collections::HashMap;

pub struct TemplateContext {
    pub file: String,
    pub date: String,
    pub year: String,
    pub author: String,
    pub project: String,
    pub filename: String,
    pub ext: String,
    pub custom: HashMap<String, String>,
}

impl TemplateContext {
    pub fn new(
        rel_path: &std::path::Path,
        date_format: &str,
        author: String,
        project: String,
        custom: HashMap<String, String>,
    ) -> Self {
        let now = Local::now();
        // Use forward slashes for header text regardless of OS
        let file = rel_path.to_string_lossy().replace('\\', "/");
        let filename = rel_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let ext = rel_path
            .extension()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        Self {
            file,
            date: now.format(date_format).to_string(),
            year: now.format("%Y").to_string(),
            author,
            project,
            filename,
            ext,
            custom,
        }
    }
}

pub fn render(template: &str, ctx: &TemplateContext) -> String {
    let mut result = template.to_string();
    result = result.replace("{{file}}", &ctx.file);
    result = result.replace("{{date}}", &ctx.date);
    result = result.replace("{{year}}", &ctx.year);
    result = result.replace("{{author}}", &ctx.author);
    result = result.replace("{{project}}", &ctx.project);
    result = result.replace("{{filename}}", &ctx.filename);
    result = result.replace("{{ext}}", &ctx.ext);
    for (k, v) in &ctx.custom {
        result = result.replace(&format!("{{{{{}}}}}", k), v);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx(file: &str) -> TemplateContext {
        TemplateContext {
            file: file.to_string(),
            date: "2026-03-19".to_string(),
            year: "2026".to_string(),
            author: "alice".to_string(),
            project: "myproject".to_string(),
            filename: "main".to_string(),
            ext: "rs".to_string(),
            custom: std::collections::HashMap::from([
                ("team".to_string(), "platform".to_string()),
            ]),
        }
    }

    #[test]
    fn render_file_var() {
        let ctx = make_ctx("src/main.rs");
        assert_eq!(render("File: {{file}}", &ctx), "File: src/main.rs");
    }

    #[test]
    fn render_all_builtins() {
        let ctx = make_ctx("src/lib.rs");
        let tpl = "{{file}} {{date}} {{year}} {{author}} {{project}} {{filename}} {{ext}}";
        let result = render(tpl, &ctx);
        assert!(result.contains("src/lib.rs"));
        assert!(result.contains("2026-03-19"));
        assert!(result.contains("2026"));
        assert!(result.contains("alice"));
        assert!(result.contains("myproject"));
        assert!(result.contains("main"));
        assert!(result.contains("rs"));
    }

    #[test]
    fn render_custom_vars() {
        let ctx = make_ctx("x.rs");
        let result = render("Team: {{team}}", &ctx);
        assert_eq!(result, "Team: platform");
    }

    #[test]
    fn render_unknown_var_passthrough() {
        let ctx = make_ctx("x.rs");
        // Unknown variables are left as-is
        let result = render("{{unknown}}", &ctx);
        assert_eq!(result, "{{unknown}}");
    }

    #[test]
    fn render_no_vars() {
        let ctx = make_ctx("x.rs");
        assert_eq!(render("just text", &ctx), "just text");
    }

    #[test]
    fn render_multiple_same_var() {
        let ctx = make_ctx("a/b.rs");
        let result = render("{{file}} and {{file}}", &ctx);
        assert_eq!(result, "a/b.rs and a/b.rs");
    }
}
