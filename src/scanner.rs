use std::path::{Path, PathBuf};

pub struct FileEntry {
    pub relative_path: String,
    pub absolute_path: PathBuf,
    pub size: u64,
}

pub fn scan_project(root: &Path) -> Result<Vec<FileEntry>, Box<dyn std::error::Error>> {
    let mut entries = Vec::new();

    let mut builder = ignore::WalkBuilder::new(root);
    builder
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .hidden(true)
        .ignore(true)
        .require_git(false)
        .sort_by_file_name(|a, b| a.cmp(b));

    for result in builder.build() {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let path = entry.path();
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);

        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        if size > 500_000 {
            continue;
        }

        entries.push(FileEntry {
            relative_path: rel_path,
            absolute_path: path.to_path_buf(),
            size,
        });
    }

    Ok(entries)
}

pub fn read_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    Ok(std::fs::read_to_string(path)?)
}

#[allow(dead_code)]
pub fn read_file_lines(
    path: &Path,
    start: usize,
    end: usize,
) -> Result<String, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();
    let start = start.min(lines.len());
    let end = end.min(lines.len());
    Ok(lines[start..end].join("\n"))
}

pub type SearchResult = Vec<(String, usize, String)>;

pub fn search_codebase(
    root: &Path,
    pattern: &str,
) -> Result<SearchResult, Box<dyn std::error::Error>> {
    let mut results = Vec::new();
    let entries = scan_project(root)?;

    for entry in &entries {
        let content = match std::fs::read_to_string(&entry.absolute_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        for (i, line) in content.lines().enumerate() {
            if line.contains(pattern) && results.len() < 50 {
                results.push((entry.relative_path.clone(), i + 1, line.trim().to_string()));
            }
        }
    }

    Ok(results)
}

/// Compute a fast, non-cryptographic hash of a file's contents using `DefaultHasher` (SipHash).
///
/// **Not for security purposes** — this is only used for change detection to skip
/// re-generating documentation for unchanged files. Do not use for integrity
/// verification, password hashing, or any security-sensitive application.
pub fn compute_file_hash(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    use std::hash::Hasher;
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.write(&buf[..n]);
    }
    Ok(format!("{:016x}", hasher.finish()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_dir() -> (std::path::PathBuf, impl FnOnce()) {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("cw_test_{}_{}", std::process::id(), n));
        std::fs::create_dir_all(&dir).unwrap();
        let d = dir.clone();
        (dir, move || {
            let _ = std::fs::remove_dir_all(&d);
        })
    }

    #[test]
    fn scan_project_finds_files() {
        let (dir, cleanup) = temp_dir();
        std::fs::write(dir.join("a.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.join("b.txt"), "hello").unwrap();
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub/c.rs"), "mod tests;").unwrap();

        let entries = scan_project(&dir).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.relative_path.as_str()).collect();

        assert!(names.contains(&"a.rs"));
        assert!(names.contains(&"b.txt"));
        assert!(names.contains(&"sub/c.rs"));
        assert_eq!(entries.len(), 3);

        cleanup();
    }

    #[test]
    fn scan_project_filters_large_files() {
        let (dir, cleanup) = temp_dir();
        std::fs::write(dir.join("small.txt"), "hi").unwrap();

        let big = "x".repeat(600_000);
        std::fs::write(dir.join("big.txt"), &big).unwrap();

        let entries = scan_project(&dir).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].relative_path, "small.txt");

        cleanup();
    }

    #[test]
    fn read_file_returns_content() {
        let (dir, cleanup) = temp_dir();
        std::fs::write(dir.join("test.txt"), "hello world\n").unwrap();

        let content = read_file(&dir.join("test.txt")).unwrap();
        assert_eq!(content, "hello world\n");

        cleanup();
    }

    #[test]
    fn read_file_lines_slice() {
        let (dir, cleanup) = temp_dir();
        std::fs::write(dir.join("lines.txt"), "line1\nline2\nline3\nline4\n").unwrap();

        let content = read_file_lines(&dir.join("lines.txt"), 1, 3).unwrap();
        assert_eq!(content, "line2\nline3");

        cleanup();
    }

    #[test]
    fn search_codebase_finds_matches() {
        let (dir, cleanup) = temp_dir();
        std::fs::write(dir.join("a.rs"), "fn hello() {\n    println!(\"hi\");\n}").unwrap();
        std::fs::write(dir.join("b.rs"), "fn world() {}").unwrap();

        let results = search_codebase(&dir, "hello").unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].0.contains("a.rs"));
        assert!(results[0].2.contains("fn hello()"));

        cleanup();
    }

    #[test]
    fn search_codebase_no_matches() {
        let (dir, cleanup) = temp_dir();
        std::fs::write(dir.join("a.rs"), "fn x() {}").unwrap();

        let results = search_codebase(&dir, "nonexistent").unwrap();
        assert!(results.is_empty());

        cleanup();
    }

    #[test]
    fn compute_file_hash_is_deterministic() {
        let (dir, cleanup) = temp_dir();
        std::fs::write(dir.join("hash.txt"), "same content").unwrap();

        let h1 = compute_file_hash(&dir.join("hash.txt")).unwrap();
        let h2 = compute_file_hash(&dir.join("hash.txt")).unwrap();
        assert_eq!(h1, h2);

        cleanup();
    }

    #[test]
    fn compute_file_hash_differs_for_different_content() {
        let (dir, cleanup) = temp_dir();
        std::fs::write(dir.join("f1.txt"), "aaa").unwrap();
        std::fs::write(dir.join("f2.txt"), "bbb").unwrap();

        let h1 = compute_file_hash(&dir.join("f1.txt")).unwrap();
        let h2 = compute_file_hash(&dir.join("f2.txt")).unwrap();
        assert_ne!(h1, h2);

        cleanup();
    }
}
