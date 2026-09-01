use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::scanner;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeIndex {
    pub version: String,
    pub project: ProjectInfo,
    pub files: Vec<FileEntry>,
    pub symbols: Vec<Symbol>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub name: String,
    pub version: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub hash: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub kind: String,
    pub name: String,
    pub file: String,
    pub line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
}

pub fn build_index(project_dir: &Path) -> Result<CodeIndex, Box<dyn std::error::Error>> {
    let (name, version, description) = read_project_metadata(project_dir);
    let files = collect_index_entries(project_dir)?;
    let symbols = extract_all_symbols(project_dir, &files);

    Ok(CodeIndex {
        version: "1.0".into(),
        project: ProjectInfo {
            name,
            version,
            description,
        },
        files,
        symbols,
    })
}

fn read_project_metadata(project_dir: &Path) -> (String, String, String) {
    let cargo_path = project_dir.join("Cargo.toml");
    let content = match std::fs::read_to_string(&cargo_path) {
        Ok(c) => c,
        Err(_) => {
            let name = project_dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".into());
            return (name, String::new(), String::new());
        }
    };

    let name = content
        .lines()
        .find(|l| l.trim().starts_with("name"))
        .and_then(|l| l.split('=').nth(1))
        .map(|v| v.trim().trim_matches('"').to_string())
        .unwrap_or_else(|| {
            project_dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".into())
        });

    let version = content
        .lines()
        .find(|l| l.trim().starts_with("version"))
        .and_then(|l| l.split('=').nth(1))
        .map(|v| v.trim().trim_matches('"').to_string())
        .unwrap_or_default();

    let description = content
        .lines()
        .find(|l| l.trim().starts_with("description"))
        .and_then(|l| l.split('=').nth(1))
        .map(|v| v.trim().trim_matches('"').to_string())
        .unwrap_or_default();

    (name, version, description)
}

fn collect_index_entries(
    project_dir: &Path,
) -> Result<Vec<FileEntry>, Box<dyn std::error::Error>> {
    let scanner_entries = scanner::scan_project(project_dir)?;
    let mut entries = Vec::new();

    for entry in scanner_entries {
        let language = detect_language(&entry.relative_path);
        let hash = scanner::compute_file_hash(&entry.absolute_path).unwrap_or_default();

        entries.push(FileEntry {
            path: entry.relative_path,
            size: entry.size,
            hash,
            language,
        });
    }

    Ok(entries)
}

fn detect_language(path: &str) -> String {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match ext {
        "rs" => "rust",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "py" => "python",
        "go" => "go",
        "java" => "java",
        "c" | "h" => "c",
        "cpp" | "cxx" | "cc" | "hpp" => "cpp",
        "rb" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "kt" | "kts" => "kotlin",
        "sh" | "bash" => "shell",
        "md" => "markdown",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "html" | "htm" => "html",
        "css" => "css",
        "sql" => "sql",
        _ => "unknown",
    }
    .to_string()
}

fn extract_all_symbols(project_dir: &Path, files: &[FileEntry]) -> Vec<Symbol> {
    let mut symbols = Vec::new();

    for file in files {
        if file.language != "rust" {
            continue;
        }

        let full_path = project_dir.join(&file.path);
        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let file_symbols = parse_symbols(&content, &file.path);
        symbols.extend(file_symbols);
    }

    symbols
}

fn parse_symbols(content: &str, file_path: &str) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    let mut doc_buf: Vec<String> = Vec::new();
    let mut in_doc = false;
    let mut current_module: Option<String> = None;

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.starts_with("/// ") || trimmed == "///" {
            in_doc = true;
            let doc_line = trimmed.strip_prefix("/// ").unwrap_or("");
            doc_buf.push(doc_line.to_string());
            continue;
        }

        if trimmed.starts_with("//! ") || trimmed == "//!" {
            in_doc = true;
            let doc_line = trimmed.strip_prefix("//! ").unwrap_or("");
            doc_buf.push(doc_line.to_string());
            continue;
        }

        if trimmed.starts_with("pub mod ") && trimmed.ends_with(';') {
            let name = trimmed
                .strip_prefix("pub mod ")
                .and_then(|s| s.strip_suffix(';'))
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if !name.contains(' ') {
                current_module = Some(name.clone());
                let doc = if doc_buf.is_empty() {
                    None
                } else {
                    Some(doc_buf.join(" "))
                };
                let signature = Some(format!("pub mod {};", name));
                symbols.push(Symbol {
                    kind: "mod".into(),
                    name,
                    file: file_path.into(),
                    line: line_num + 1,
                    doc,
                    signature,
                    module: None,
                });
            }
            doc_buf.clear();
            in_doc = false;
            continue;
        }

        if let Some((kind, rest)) = detect_symbol(trimmed) {
            if let Some(name) = extract_symbol_name(rest) {
                let doc = if doc_buf.is_empty() {
                    None
                } else {
                    Some(doc_buf.join(" "))
                };
                let signature = extract_signature(trimmed);
                symbols.push(Symbol {
                    kind: kind.into(),
                    name,
                    file: file_path.into(),
                    line: line_num + 1,
                    doc,
                    signature,
                    module: current_module.clone(),
                });
            }
            doc_buf.clear();
            in_doc = false;
            continue;
        }

        if in_doc && !trimmed.is_empty() && !trimmed.starts_with("//") {
            in_doc = false;
            doc_buf.clear();
        }

        if trimmed.is_empty() || trimmed.starts_with("use ") || trimmed.starts_with("pub use ") {
            current_module = None;
        }
    }

    symbols
}

fn detect_symbol(line: &str) -> Option<(&str, &str)> {
    if !line.starts_with("pub ") && !line.starts_with("pub(") {
        return None;
    }

    let line = line.trim_start_matches("pub ");
    let line = line.trim_start_matches("pub(crate) ");
    let line = line.trim_start_matches("pub(super) ");

    if let Some(rest) = line.strip_prefix("fn ") {
        Some(("fn", rest))
    } else if let Some(rest) = line.strip_prefix("struct ") {
        Some(("struct", rest))
    } else if let Some(rest) = line.strip_prefix("enum ") {
        Some(("enum", rest))
    } else if let Some(rest) = line.strip_prefix("trait ") {
        Some(("trait", rest))
    } else if let Some(rest) = line.strip_prefix("type ") {
        Some(("type", rest))
    } else if let Some(rest) = line.strip_prefix("const ") {
        Some(("const", rest))
    } else if let Some(rest) = line.strip_prefix("static ") {
        Some(("static", rest))
    } else if let Some(rest) = line.strip_prefix("mod ") {
        Some(("mod", rest))
    } else {
        None
    }
}

fn extract_symbol_name(rest: &str) -> Option<String> {
    let name = rest
        .split(['<', '(', '{', ';', ':'])
        .next()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn extract_signature(line: &str) -> Option<String> {
    let trimmed = line.trim();

    if trimmed.contains('{') || trimmed.contains(';') {
        let end = if let Some(pos) = trimmed.find('{') {
            pos
        } else if let Some(pos) = trimmed.find(';') {
            pos
        } else {
            trimmed.len()
        };
        let sig = trimmed[..end].trim().to_string();
        if sig.contains("pub ") || sig.contains("fn ") || sig.contains("struct ") {
            Some(sig)
        } else {
            None
        }
    } else {
        None
    }
}

pub fn load_index(project_dir: &Path) -> Result<CodeIndex, Box<dyn std::error::Error>> {
    let path = project_dir.join("wakawiki").join("index.json");
    let content = std::fs::read_to_string(&path)?;
    let index: CodeIndex = serde_json::from_str(&content)?;
    Ok(index)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub symbols: Vec<Symbol>,
    pub files: Vec<FileEntry>,
}

pub fn query_index(index: &CodeIndex, pattern: &str) -> QueryResult {
    let pattern_lower = pattern.to_lowercase();

    let symbols: Vec<Symbol> = index
        .symbols
        .iter()
        .filter(|s| {
            s.name.to_lowercase().contains(&pattern_lower)
                || s.file.to_lowercase().contains(&pattern_lower)
                || s.kind.to_lowercase().contains(&pattern_lower)
                || s.doc
                    .as_ref()
                    .map(|d| d.to_lowercase().contains(&pattern_lower))
                    .unwrap_or(false)
                || s.signature
                    .as_ref()
                    .map(|sig| sig.to_lowercase().contains(&pattern_lower))
                    .unwrap_or(false)
        })
        .cloned()
        .collect();

    let files: Vec<FileEntry> = index
        .files
        .iter()
        .filter(|f| {
            f.path.to_lowercase().contains(&pattern_lower)
                || f.language.to_lowercase().contains(&pattern_lower)
        })
        .cloned()
        .collect();

    QueryResult { symbols, files }
}

pub fn format_query_result(result: &QueryResult, pattern: &str) -> String {
    let mut output = String::new();

    if result.symbols.is_empty() && result.files.is_empty() {
        output.push_str(&format!("No results for \"{pattern}\"\n"));
        return output;
    }

    if !result.symbols.is_empty() {
        output.push_str(&format!("Symbols ({}):\n", result.symbols.len()));
        for s in &result.symbols {
            let doc = s
                .doc
                .as_ref()
                .map(|d| format!(" — {}", d.lines().next().unwrap_or("")))
                .unwrap_or_default();
            output.push_str(&format!(
                "  {} {} {}:{}{}\n",
                s.kind, s.name, s.file, s.line, doc
            ));
        }
    }

    if !result.files.is_empty() {
        if !result.symbols.is_empty() {
            output.push('\n');
        }
        output.push_str(&format!("Files ({}):\n", result.files.len()));
        for f in &result.files {
            let kb = f.size / 1024;
            output.push_str(&format!(
                "  {} [{}] ({} KB)\n",
                f.path,
                f.language,
                if kb == 0 { 1 } else { kb }
            ));
        }
    }

    output
}

pub fn save_index(project_dir: &Path, index: &CodeIndex) -> Result<String, Box<dyn std::error::Error>> {
    let wakawiki_dir = project_dir.join("wakawiki");
    std::fs::create_dir_all(&wakawiki_dir)?;

    let json = serde_json::to_string_pretty(index)?;
    let path = wakawiki_dir.join("index.json");
    std::fs::write(&path, &json)?;

    Ok(path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_language_works() {
        assert_eq!(detect_language("main.rs"), "rust");
        assert_eq!(detect_language("app.js"), "javascript");
        assert_eq!(detect_language("utils.ts"), "typescript");
        assert_eq!(detect_language("script.py"), "python");
        assert_eq!(detect_language("unknown.xyz"), "unknown");
    }

    #[test]
    fn parse_symbols_finds_pub_fn() {
        let content = "/// A greet function\npub fn greet(name: &str) -> String {\n    format!(\"Hello, {name}\")\n}\n";
        let symbols = parse_symbols(content, "test.rs");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].kind, "fn");
        assert_eq!(symbols[0].name, "greet");
        assert_eq!(symbols[0].doc, Some("A greet function".into()));
    }

    #[test]
    fn parse_symbols_finds_struct() {
        let content = "/// Main config\npub struct Config {\n    pub name: String,\n}\n";
        let symbols = parse_symbols(content, "test.rs");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].kind, "struct");
        assert_eq!(symbols[0].name, "Config");
    }

    #[test]
    fn parse_symbols_finds_multiple() {
        let content = "pub fn a() {}\npub struct B {}\npub enum C {}\n";
        let symbols = parse_symbols(content, "test.rs");
        assert_eq!(symbols.len(), 3);
    }

    #[test]
    fn parse_symbols_ignores_private() {
        let content = "fn private() {}\nstruct Hidden {}\n";
        let symbols = parse_symbols(content, "test.rs");
        assert!(symbols.is_empty());
    }

    #[test]
    fn extract_symbol_name_works() {
        assert_eq!(extract_symbol_name("greet(name: &str)"), Some("greet".into()));
        assert_eq!(extract_symbol_name("Config {"), Some("Config".into()));
        assert_eq!(extract_symbol_name("Result<T, E>"), Some("Result".into()));
    }
}
