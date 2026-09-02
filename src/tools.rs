use std::path::{Path, PathBuf};

/// Safely join a relative path onto a base directory.
/// Returns `Err` if the resulting path escapes the base directory.
pub fn safe_join(base: &Path, relative: &str) -> Result<PathBuf, String> {
    let relative = relative.trim_start_matches('/');
    if relative.contains("..") {
        return Err(format!(
            "Path traversal rejected: '{relative}' contains '..' component"
        ));
    }
    let joined = base.join(relative);
    let canonical_base =
        std::fs::canonicalize(base).map_err(|e| format!("Cannot resolve base path: {e}"))?;
    let canonical_joined =
        std::fs::canonicalize(&joined).unwrap_or_else(|_| canonical_base.join(relative));
    if !canonical_joined.starts_with(&canonical_base) {
        return Err(format!(
            "Path traversal rejected: '{}' escapes base directory",
            joined.display()
        ));
    }
    Ok(joined)
}

pub mod fs {
    use super::safe_join;
    use crate::scanner;
    use std::path::{Path, PathBuf};

    pub fn list_files(project_dir: &Path, subpath: &str) -> String {
        let dir_to_list = if subpath.is_empty() {
            project_dir.to_path_buf()
        } else {
            match safe_join(project_dir, subpath) {
                Ok(p) => p,
                Err(e) => return e,
            }
        };

        if !dir_to_list.exists() {
            return format!("Error: path does not exist: {:?}", dir_to_list);
        }

        match scanner::scan_project(&dir_to_list) {
            Ok(entries) => {
                if entries.is_empty() {
                    format!("Directory is empty: {:?}", dir_to_list)
                } else {
                    let mut lines: Vec<String> = Vec::new();
                    for e in &entries {
                        let kb = e.size / 1024;
                        lines.push(format!(
                            "{} ({} KB)",
                            e.relative_path,
                            if kb == 0 { 1 } else { kb }
                        ));
                    }
                    lines.join("\n")
                }
            }
            Err(e) => format!("Error listing directory: {e}"),
        }
    }

    pub fn read_file(project_dir: &Path, filepath: &str) -> String {
        if filepath.is_empty() {
            return "Error: no path provided".into();
        }

        let full_path = match safe_join(project_dir, filepath) {
            Ok(p) => p,
            Err(e) => return e,
        };
        match scanner::read_file(&full_path) {
            Ok(content) => {
                let line_count = content.lines().count();
                if content.len() > 100_000 {
                    format!(
                        "File too large ({} lines, {} bytes). Here are the first 500 lines:\n\n{}",
                        line_count,
                        content.len(),
                        content.lines().take(500).collect::<Vec<_>>().join("\n")
                    )
                } else {
                    format!(
                        "File: {filepath} ({} lines, {} bytes)\n\n{content}",
                        line_count,
                        content.len()
                    )
                }
            }
            Err(e) => format!("Error reading file: {e}"),
        }
    }

    pub fn search(project_dir: &Path, pattern: &str) -> String {
        if pattern.is_empty() {
            return "Error: no pattern provided".into();
        }

        match scanner::search_codebase(project_dir, pattern) {
            Ok(results) => {
                if results.is_empty() {
                    format!("No matches found for '{pattern}'")
                } else {
                    let mut output = String::new();
                    for (file, line, text) in &results {
                        output.push_str(&format!("{file}:{line}: {text}\n"));
                    }
                    output
                }
            }
            Err(e) => format!("Error searching: {e}"),
        }
    }

    pub fn write_doc_file(
        wakawiki_dir: &Path,
        relative_path: &str,
        content: &str,
    ) -> Result<PathBuf, String> {
        let full_path = safe_join(wakawiki_dir, relative_path)?;
        if let Some(parent) = full_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&full_path, content);
        Ok(full_path)
    }
}

pub mod index {
    use crate::index::{self, CodeIndex};
    use crate::vector::{self, VectorStore};

    pub fn query_symbols(index: &CodeIndex, pattern: &str) -> String {
        let result = index::query_index(index, pattern);
        index::format_query_result(&result, pattern)
    }

    pub fn get_symbol(index: &CodeIndex, name: &str, file: Option<&str>) -> Result<String, String> {
        let symbols: Vec<_> = index
            .symbols
            .iter()
            .filter(|s| s.name == name && file.map(|f| s.file == f).unwrap_or(true))
            .collect();

        if symbols.is_empty() {
            return Err(format!("Symbol \"{name}\" not found"));
        }

        let text = symbols
            .iter()
            .map(|s| {
                let doc = s
                    .doc
                    .as_ref()
                    .map(|d| format!("\n{}", d))
                    .unwrap_or_default();
                let sig = s
                    .signature
                    .as_ref()
                    .map(|s| format!("\nSignature: {}", s))
                    .unwrap_or_default();
                format!(
                    "{} {} in {}:{}{}{}",
                    s.kind, s.name, s.file, s.line, sig, doc
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        Ok(text)
    }

    pub fn list_files_text(index: &CodeIndex, language: Option<&str>) -> String {
        let files: Vec<_> = index
            .files
            .iter()
            .filter(|f| language.map(|l| f.language == l).unwrap_or(true))
            .collect();

        files
            .iter()
            .map(|f| {
                let kb = f.size / 1024;
                format!(
                    "{} [{}] ({} KB)",
                    f.path,
                    f.language,
                    if kb == 0 { 1 } else { kb }
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn get_file_info(index: &CodeIndex, path: &str) -> Result<String, String> {
        let file = index.files.iter().find(|f| f.path == path);

        match file {
            Some(f) => {
                let symbols: Vec<_> = index.symbols.iter().filter(|s| s.file == path).collect();

                let mut text = format!(
                    "File: {}\nLanguage: {}\nSize: {} bytes\nHash: {}",
                    f.path, f.language, f.size, f.hash
                );

                if !symbols.is_empty() {
                    text.push_str("\n\nSymbols:");
                    for s in &symbols {
                        text.push_str(&format!("\n  {} {} (line {})", s.kind, s.name, s.line));
                    }
                }

                Ok(text)
            }
            None => Err(format!("File \"{path}\" not found in index")),
        }
    }

    pub fn get_project_info(index: &CodeIndex) -> String {
        format!(
            "Project: {}\nVersion: {}\nDescription: {}\nFiles: {}\nSymbols: {}",
            index.project.name,
            index.project.version,
            index.project.description,
            index.files.len(),
            index.symbols.len()
        )
    }

    pub fn semantic_search_text(store: &VectorStore, query: &str, top_k: usize) -> String {
        let results = store.search(query, top_k);
        vector::format_semantic_results(&results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir() -> (std::path::PathBuf, impl FnOnce()) {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("cw_tools_{}_{}", std::process::id(), n));
        std::fs::create_dir_all(&dir).unwrap();
        let d = dir.clone();
        (dir, move || {
            let _ = std::fs::remove_dir_all(&d);
        })
    }

    #[test]
    fn safe_join_allows_valid() {
        let (dir, cleanup) = temp_dir();
        assert!(safe_join(&dir, "src/main.rs").is_ok());
        cleanup();
    }

    #[test]
    fn safe_join_rejects_traversal() {
        let (dir, cleanup) = temp_dir();
        assert!(safe_join(&dir, "../../etc/passwd").is_err());
        cleanup();
    }

    #[test]
    fn fs_list_files_rejects_traversal() {
        let (dir, cleanup) = temp_dir();
        let result = fs::list_files(&dir, "../../etc/passwd");
        assert!(result.contains("traversal"));
        cleanup();
    }

    #[test]
    fn fs_read_file_rejects_traversal() {
        let (dir, cleanup) = temp_dir();
        let result = fs::read_file(&dir, "../../etc/passwd");
        assert!(result.contains("traversal"));
        cleanup();
    }

    #[test]
    fn fs_write_doc_file_rejects_traversal() {
        let (dir, cleanup) = temp_dir();
        let result = fs::write_doc_file(&dir, "../../evil.md", "x");
        assert!(result.is_err());
        cleanup();
    }
}
