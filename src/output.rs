use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiMeta {
    pub file_hashes: HashMap<String, String>,
}

pub fn load_wiki_meta(wakawiki_dir: &Path) -> WikiMeta {
    let meta_path = wakawiki_dir.join(".wakawiki.json");
    match std::fs::read_to_string(&meta_path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(meta) => meta,
            Err(_) => WikiMeta {
                file_hashes: HashMap::new(),
            },
        },
        Err(_) => WikiMeta {
            file_hashes: HashMap::new(),
        },
    }
}

pub fn save_wiki_meta(wakawiki_dir: &Path, meta: &WikiMeta) {
    let meta_path = wakawiki_dir.join(".wakawiki.json");
    if let Ok(json) = serde_json::to_string_pretty(meta) {
        let _ = std::fs::write(meta_path, json);
    }
}

pub fn write_doc(wakawiki_dir: &Path, relative_path: &str, content: &str) -> PathBuf {
    let full_path = wakawiki_dir.join(relative_path);
    if let Some(parent) = full_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&full_path, content);
    full_path
}

pub fn append_agents_reference(project_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let agents_path = project_dir.join("AGENTS.md");
    let reference = "\n\n<!-- wakawiki:start -->\n## wakawiki Documentation\n\nThis repository has wakawiki-generated documentation in the `wakawiki/` directory.\nWhen you need context about the codebase, reference the files in `wakawiki/`:\n- `wakawiki/index.md` — Project overview\n- `wakawiki/architecture.md` — Architecture and design\n\nYou can also use the wakawiki CLI to update documentation:\n```bash\nwakawiki --update\n```\n<!-- wakawiki:end -->\n";

    let existing = if agents_path.exists() {
        std::fs::read_to_string(&agents_path).unwrap_or_default()
    } else {
        String::new()
    };

    let start_marker = "<!-- wakawiki:start -->";
    let end_marker = "<!-- wakawiki:end -->";

    let new_content = if let (Some(start), Some(end)) =
        (existing.find(start_marker), existing.find(end_marker))
    {
        format!(
            "{}{}{}",
            &existing[..start],
            reference.trim(),
            &existing[end + end_marker.len()..]
        )
    } else {
        format!(
            "{}<!-- wakawiki:start -->\n## wakawiki Documentation\n\nThis repository has wakawiki-generated documentation in the `wakawiki/` directory.\nWhen you need context about the codebase, reference the files in `wakawiki/`:\n- `wakawiki/index.md` — Project overview\n- `wakawiki/architecture.md` — Architecture and design\n\nYou can also use the wakawiki CLI to update documentation:\n```bash\nwakawiki --update\n```\n<!-- wakawiki:end -->\n",
            existing
        )
    };

    std::fs::write(&agents_path, new_content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir() -> (std::path::PathBuf, impl FnOnce()) {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("cw_out_{}_{}", std::process::id(), n));
        std::fs::create_dir_all(&dir).unwrap();
        let d = dir.clone();
        (dir, move || {
            let _ = std::fs::remove_dir_all(&d);
        })
    }

    #[test]
    fn write_doc_creates_file() {
        let (dir, cleanup) = temp_dir();
        let path = write_doc(&dir, "index.md", "# Hello");
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "# Hello");
        cleanup();
    }

    #[test]
    fn write_doc_creates_parent_dirs() {
        let (dir, cleanup) = temp_dir();
        let path = write_doc(&dir, "sub/deep/file.md", "content");
        assert!(path.exists());
        assert!(path.to_string_lossy().contains("sub/deep"));
        cleanup();
    }

    #[test]
    fn load_wiki_meta_empty_dir() {
        let (dir, cleanup) = temp_dir();
        let meta = load_wiki_meta(&dir);
        assert!(meta.file_hashes.is_empty());
        cleanup();
    }

    #[test]
    fn save_and_load_wiki_meta_roundtrip() {
        let (dir, cleanup) = temp_dir();
        let mut meta = WikiMeta {
            file_hashes: HashMap::new(),
        };
        meta.file_hashes.insert("index.md".into(), "abc123".into());

        save_wiki_meta(&dir, &meta);
        let loaded = load_wiki_meta(&dir);
        assert_eq!(loaded.file_hashes.get("index.md").unwrap(), "abc123");
        cleanup();
    }

    #[test]
    fn append_agents_reference_creates_file() {
        let (dir, cleanup) = temp_dir();
        append_agents_reference(&dir).unwrap();
        let content = std::fs::read_to_string(dir.join("AGENTS.md")).unwrap();
        assert!(content.contains("wakawiki:start"));
        assert!(content.contains("wakawiki --update"));
        cleanup();
    }

    #[test]
    fn append_agents_reference_idempotent() {
        let (dir, cleanup) = temp_dir();
        append_agents_reference(&dir).unwrap();
        append_agents_reference(&dir).unwrap();
        let content = std::fs::read_to_string(dir.join("AGENTS.md")).unwrap();
        let count = content.matches("wakawiki:start").count();
        assert_eq!(count, 1);
        cleanup();
    }

    #[test]
    fn append_agents_reference_preserves_existing_content() {
        let (dir, cleanup) = temp_dir();
        std::fs::write(dir.join("AGENTS.md"), "# My Project\n").unwrap();
        append_agents_reference(&dir).unwrap();
        let content = std::fs::read_to_string(dir.join("AGENTS.md")).unwrap();
        assert!(content.contains("# My Project"));
        assert!(content.contains("wakawiki:start"));
        cleanup();
    }
}
