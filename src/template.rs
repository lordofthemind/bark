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
