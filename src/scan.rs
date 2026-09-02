use std::collections::BTreeMap;
use std::path::Path;

use crate::output;
use crate::scanner;

#[derive(Debug, Clone)]
struct DocItem {
    doc: Vec<String>,
    name: String,
    kind: String,
}

#[derive(Debug, Clone)]
struct ModuleInfo {
    submodules: Vec<String>,
    items: Vec<DocItem>,
}

pub fn run(project_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let wakawiki_dir = project_dir.join("wakawiki");
    std::fs::create_dir_all(&wakawiki_dir)?;

    let mut wiki_meta = output::load_wiki_meta(&wakawiki_dir);

    let name = project_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".into());

    let (version, description) = read_cargo_toml(project_dir);

    let deps = read_dependencies(project_dir);

    let files = collect_source_files(project_dir);

    let mut modules: BTreeMap<String, ModuleInfo> = BTreeMap::new();
    for rel_path in &files {
        if let Some(ext) = Path::new(rel_path).extension() {
            if ext == "rs" {
                if let Ok(content) = std::fs::read_to_string(project_dir.join(rel_path)) {
                    let info = parse_rust_file(&content);
                    modules.insert(rel_path.clone(), info);
                }
            }
        }
    }

    let dir_tree = build_dir_tree(&files);

    let index_md = generate_index(&name, &version, &description, &deps, &dir_tree);
    let index_path = output::write_doc(&wakawiki_dir, "index.md", &index_md)?;
    if let Ok(hash) = scanner::compute_file_hash(&index_path) {
        wiki_meta.file_hashes.insert("index.md".into(), hash);
    }
    println!("  -> write_doc: index.md ({} bytes)", index_md.len());

    let api_md = generate_api(&modules);
    let api_path = output::write_doc(&wakawiki_dir, "architecture.md", &api_md)?;
    if let Ok(hash) = scanner::compute_file_hash(&api_path) {
        wiki_meta.file_hashes.insert("architecture.md".into(), hash);
    }
    println!("  -> write_doc: architecture.md ({} bytes)", api_md.len());

    output::save_wiki_meta(&wakawiki_dir, &wiki_meta);

    let _ = output::append_agents_reference(project_dir);

    println!("\nDocumentation written to wakawiki/ (heuristic scan)");
    Ok(())
}

fn read_cargo_toml(project_dir: &Path) -> (String, String) {
    let path = project_dir.join("Cargo.toml");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return (String::new(), String::new()),
    };

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

    (version, description)
}

fn read_dependencies(project_dir: &Path) -> Vec<(String, String)> {
    let path = project_dir.join("Cargo.toml");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let mut deps = Vec::new();
    let mut in_deps = false;
    for line in content.lines() {
        if line.trim() == "[dependencies]" {
            in_deps = true;
            continue;
        }
        if in_deps {
            if line.trim().is_empty() || line.trim().starts_with('[') {
                break;
            }
            let (name, rest) = match line.split_once('=') {
                Some((n, r)) => (n.trim().to_string(), r.trim()),
                None => continue,
            };
            let ver = if rest.trim().starts_with('{') {
                rest.split('"').nth(1).unwrap_or("*").to_string()
            } else {
                rest.split(',')
                    .next()
                    .map(|v| v.trim().trim_matches('"').to_string())
                    .unwrap_or_else(|| "*".into())
            };
            deps.push((name, ver));
        }
    }
    deps
}

fn collect_source_files(project_dir: &Path) -> Vec<String> {
    let mut files = Vec::new();
    let mut builder = ignore::WalkBuilder::new(project_dir);
    builder
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .ignore(true)
        .hidden(false)
        .require_git(false)
        .sort_by_file_name(|a, b| a.cmp(b))
        .filter_entry(move |entry| {
            let path = entry.path();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            name != ".git" && name != "wakawiki"
        });

    for result in builder.build() {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        if size > 500_000 {
            continue;
        }
        let rel_path = entry
            .path()
            .strip_prefix(project_dir)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .to_string();
        if rel_path.starts_with("wakawiki/") || rel_path.ends_with(".lock") {
            continue;
        }
        files.push(rel_path);
    }
    files
}

fn build_dir_tree(files: &[String]) -> Vec<String> {
    let mut dirs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for f in files {
        if let Some(parent) = Path::new(f).parent() {
            let parent = parent.to_string_lossy().to_string();
            let name = Path::new(f)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            dirs.entry(parent).or_default().push(name);
        }
    }

    let mut lines = Vec::new();
    lines.push("```".into());
    let mut sorted_dirs: Vec<_> = dirs.iter().collect();
    sorted_dirs.sort_by(|a, b| a.0.cmp(b.0));

    for (dir, entries) in &sorted_dirs {
        let display = if dir.is_empty() { "." } else { dir };
        lines.push(format!("{}/", display));
        for entry in *entries {
            lines.push(format!("  {}", entry));
        }
    }
    lines.push("```".into());
    lines
}

fn parse_rust_file(content: &str) -> ModuleInfo {
    let mut info = ModuleInfo {
        submodules: Vec::new(),
        items: Vec::new(),
    };

    let mut doc_buf: Vec<String> = Vec::new();
    let mut in_doc = false;

    for line in content.lines() {
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

        if trimmed.starts_with("mod ") && trimmed.ends_with(';') {
            let name = trimmed
                .strip_prefix("mod ")
                .and_then(|s| s.strip_suffix(';'))
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if !name.contains(' ') {
                info.submodules.push(name);
            }
            doc_buf.clear();
            in_doc = false;
            continue;
        }

        if trimmed.starts_with("pub mod ") && trimmed.ends_with(';') {
            let name = trimmed
                .strip_prefix("pub mod ")
                .and_then(|s| s.strip_suffix(';'))
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if !name.contains(' ') {
                info.submodules.push(name);
            }
            doc_buf.clear();
            in_doc = false;
            continue;
        }

        if let Some((kind, rest)) = detect_item(trimmed) {
            if let Some(name) = extract_name(rest) {
                let doc = std::mem::take(&mut doc_buf);
                info.items.push(DocItem {
                    doc,
                    name,
                    kind: kind.to_string(),
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
    }

    info
}

fn detect_item(line: &str) -> Option<(&str, &str)> {
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

fn extract_name(rest: &str) -> Option<String> {
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

fn generate_index(
    name: &str,
    version: &str,
    description: &str,
    deps: &[(String, String)],
    dir_tree: &[String],
) -> String {
    let mut md = String::new();
    md.push_str(&format!("# {}\n\n", name));

    if !description.is_empty() {
        md.push_str(&format!("_{}_\n\n", description));
    }

    md.push_str("## Project Info\n\n");
    md.push_str(&format!("- **Name:** {}\n", name));
    if !version.is_empty() {
        md.push_str(&format!("- **Version:** {}\n", version));
    }
    md.push_str("- **Files:** generated by heuristic scan (no LLM)\n\n");

    if !deps.is_empty() {
        md.push_str("## Dependencies\n\n");
        md.push_str("| Crate | Version |\n");
        md.push_str("|-------|--------|\n");
        for (name, ver) in deps {
            md.push_str(&format!("| `{}` | {} |\n", name, ver));
        }
        md.push('\n');
    }

    md.push_str("## Directory Structure\n\n");
    for line in dir_tree {
        md.push_str(line);
        md.push('\n');
    }

    md
}

fn generate_api(modules: &BTreeMap<String, ModuleInfo>) -> String {
    let mut md = String::from("# Architecture & API Reference\n\n");
    md.push_str("_Generated by heuristic static analysis. No LLM involved._\n\n");

    for (path, info) in modules {
        if info.submodules.is_empty() && info.items.is_empty() {
            continue;
        }

        md.push_str(&format!("## `{}`\n\n", path));

        if !info.submodules.is_empty() {
            md.push_str("**Submodules:** ");
            let names: Vec<_> = info.submodules.iter().map(|m| format!("`{}`", m)).collect();
            md.push_str(&names.join(", "));
            md.push_str("\n\n");
        }

        if !info.items.is_empty() {
            md.push_str("| Kind | Name | Description |\n");
            md.push_str("|------|------|-------------|\n");
            for item in &info.items {
                let desc = if item.doc.is_empty() {
                    "—".into()
                } else {
                    item.doc.first().cloned().unwrap_or_default()
                };
                md.push_str(&format!(
                    "| `{}` | `{}` | {} |\n",
                    item.kind, item.name, desc
                ));
            }
            md.push('\n');
        }
    }

    md
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_module() {
        let src = "/// A greet function\npub fn greet(name: &str) -> String {\n    format!(\"Hello, {name}\")\n}\n";
        let info = parse_rust_file(src);
        assert_eq!(info.items.len(), 1);
        assert_eq!(info.items[0].kind, "fn");
        assert_eq!(info.items[0].name, "greet");
        assert_eq!(info.items[0].doc, vec!["A greet function"]);
    }

    #[test]
    fn parse_multiple_items() {
        let src = "/// Main struct\npub struct Config {\n    pub name: String,\n}\n\npub fn do_thing() {}\n";
        let info = parse_rust_file(src);
        assert_eq!(info.items.len(), 2);
        assert_eq!(info.items[0].kind, "struct");
        assert_eq!(info.items[0].name, "Config");
        assert_eq!(info.items[1].kind, "fn");
        assert_eq!(info.items[1].name, "do_thing");
    }

    #[test]
    fn parse_submodules() {
        let src = "mod scanner;\nmod output;\npub mod provider;\n";
        let info = parse_rust_file(src);
        assert_eq!(info.submodules.len(), 3);
        assert!(info.submodules.contains(&"scanner".to_string()));
        assert!(info.submodules.contains(&"output".to_string()));
        assert!(info.submodules.contains(&"provider".to_string()));
    }

    #[test]
    fn parse_no_doc_item() {
        let src = "pub fn no_doc() -> bool { true }\n";
        let info = parse_rust_file(src);
        assert_eq!(info.items.len(), 1);
        assert!(info.items[0].doc.is_empty());
    }

    #[test]
    fn parse_enum_with_doc() {
        let src =
            "/// Represents providers\npub enum LlmProvider {\n    OpenAi,\n    Anthropic,\n}\n";
        let info = parse_rust_file(src);
        assert_eq!(info.items.len(), 1);
        assert_eq!(info.items[0].kind, "enum");
        assert_eq!(info.items[0].name, "LlmProvider");
        assert_eq!(info.items[0].doc, vec!["Represents providers"]);
    }

    #[test]
    fn parse_trait_item() {
        let src = "pub trait Serializable {\n    fn serialize(&self) -> String;\n}\n";
        let info = parse_rust_file(src);
        assert_eq!(info.items.len(), 1);
        assert_eq!(info.items[0].kind, "trait");
        assert_eq!(info.items[0].name, "Serializable");
    }

    #[test]
    fn parse_type_alias() {
        let src = "pub type Result<T> = std::result::Result<T, Box<dyn Error>>;\n";
        let info = parse_rust_file(src);
        assert_eq!(info.items.len(), 1);
        assert_eq!(info.items[0].kind, "type");
        assert_eq!(info.items[0].name, "Result");
    }

    #[test]
    fn read_cargo_toml_works() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("wakawiki_scan_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut f = std::fs::File::create(dir.join("Cargo.toml")).unwrap();
        writeln!(f, "[package]").unwrap();
        writeln!(f, "name = \"test-crate\"").unwrap();
        writeln!(f, "version = \"0.2.0\"").unwrap();
        writeln!(f, "description = \"A test crate\"").unwrap();
        writeln!(f).unwrap();
        writeln!(f, "[dependencies]").unwrap();
        writeln!(f, "serde = \"1.0\"").unwrap();
        writeln!(f, "tokio = {{ version = \"1\", features = [\"full\"] }}").unwrap();

        let (version, desc) = read_cargo_toml(&dir);
        assert_eq!(version, "0.2.0");
        assert_eq!(desc, "A test crate");

        let deps = read_dependencies(&dir);
        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0].0, "serde");
        assert_eq!(deps[0].1, "1.0");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn detect_item_pub_crate() {
        let result = detect_item("pub(crate) fn internal() {}");
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, "fn");
    }

    #[test]
    fn detect_item_generic_fn() {
        let result = detect_item("pub fn parse<T: Display>(val: T) -> String {");
        assert!(result.is_some());
        let (kind, rest) = result.unwrap();
        assert_eq!(kind, "fn");
        assert_eq!(extract_name(rest).unwrap(), "parse");
    }
}
