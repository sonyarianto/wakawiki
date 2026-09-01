use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::index::CodeIndex;

const EMBEDDING_DIM: usize = 128;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorStore {
    pub embeddings: Vec<EmbeddingEntry>,
    pub vocabulary: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingEntry {
    pub id: String,
    pub text: String,
    pub kind: String,
    pub file: String,
    pub line: usize,
    pub vector: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticResult {
    pub entry: EmbeddingEntry,
    pub score: f32,
}

impl VectorStore {
    pub fn new() -> Self {
        Self {
            embeddings: Vec::new(),
            vocabulary: HashMap::new(),
        }
    }

    pub fn build_from_index(&mut self, index: &CodeIndex) {
        let mut all_tokens: HashMap<String, usize> = HashMap::new();

        for symbol in &index.symbols {
            let text = format!(
                "{} {} {} {}",
                symbol.name,
                symbol.kind,
                symbol.file,
                symbol.doc.as_deref().unwrap_or("")
            );
            let tokens = tokenize(&text);
            for token in tokens {
                *all_tokens.entry(token).or_insert(0) += 1;
            }
        }

        let total_docs = index.symbols.len() as f32;
        for (token, count) in &all_tokens {
            let _idf = (total_docs / *count as f32).ln();
            self.vocabulary.insert(token.clone(), self.vocabulary.len());
        }

        for symbol in &index.symbols {
            let text = format!(
                "{} {} {} {} {}",
                symbol.name,
                symbol.kind,
                symbol.file,
                symbol.line,
                symbol.doc.as_deref().unwrap_or("")
            );

            let vector = compute_tfidf(&text, &self.vocabulary, &all_tokens, total_docs);
            let id = format!("{}:{}", symbol.file, symbol.line);

            self.embeddings.push(EmbeddingEntry {
                id,
                text: symbol.name.clone(),
                kind: symbol.kind.clone(),
                file: symbol.file.clone(),
                line: symbol.line,
                vector,
            });
        }
    }

    pub fn search(&self, query: &str, top_k: usize) -> Vec<SemanticResult> {
        let total_docs = self.embeddings.len() as f32;
        let query_vector = compute_tfidf_simple(query, &self.vocabulary, total_docs);

        let mut results: Vec<SemanticResult> = self
            .embeddings
            .iter()
            .map(|entry| {
                let score = cosine_similarity(&query_vector, &entry.vector);
                SemanticResult {
                    entry: entry.clone(),
                    score,
                }
            })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }

    pub fn save(&self, project_dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
        let wakawiki_dir = project_dir.join("wakawiki");
        std::fs::create_dir_all(&wakawiki_dir)?;

        let json = serde_json::to_string_pretty(self)?;
        let path = wakawiki_dir.join("embeddings.json");
        std::fs::write(&path, &json)?;

        Ok(path.to_string_lossy().to_string())
    }

    pub fn load(project_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let path = project_dir.join("wakawiki").join("embeddings.json");
        let content = std::fs::read_to_string(&path)?;
        let store: VectorStore = serde_json::from_str(&content)?;
        Ok(store)
    }
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.len() > 2)
        .map(|s| s.to_string())
        .collect()
}

fn compute_tfidf(
    text: &str,
    vocabulary: &HashMap<String, usize>,
    doc_freq: &HashMap<String, usize>,
    total_docs: f32,
) -> Vec<f32> {
    let mut vector = vec![0.0; EMBEDDING_DIM];
    let tokens = tokenize(text);

    let mut term_freq: HashMap<String, usize> = HashMap::new();
    for token in &tokens {
        *term_freq.entry(token.clone()).or_insert(0) += 1;
    }

    for (token, freq) in &term_freq {
        if let Some(&_idx) = vocabulary.get(token) {
            let tf = *freq as f32 / tokens.len() as f32;
            let df = doc_freq.get(token).copied().unwrap_or(1) as f32;
            let idf = (total_docs / df).ln();
            let tfidf = tf * idf;

            let hash = simple_hash(token) % EMBEDDING_DIM;
            vector[hash] += tfidf;

            let hash2 = (simple_hash(token) + 7) % EMBEDDING_DIM;
            vector[hash2] += tfidf * 0.5;
        }
    }

    normalize(&vector)
}

fn compute_tfidf_simple(
    text: &str,
    vocabulary: &HashMap<String, usize>,
    _total_docs: f32,
) -> Vec<f32> {
    let mut vector = vec![0.0; EMBEDDING_DIM];
    let tokens = tokenize(text);

    for token in &tokens {
        if vocabulary.contains_key(token) {
            let hash = simple_hash(token) % EMBEDDING_DIM;
            vector[hash] += 1.0;

            let hash2 = (simple_hash(token) + 7) % EMBEDDING_DIM;
            vector[hash2] += 0.5;
        }
    }

    normalize(&vector)
}

fn simple_hash(s: &str) -> usize {
    let mut hash: usize = 5381;
    for byte in s.bytes() {
        hash = ((hash << 5).wrapping_add(hash)) ^ byte as usize;
    }
    hash
}

fn normalize(v: &[f32]) -> Vec<f32> {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm == 0.0 {
        v.to_vec()
    } else {
        v.iter().map(|x| x / norm).collect()
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

pub fn format_semantic_results(results: &[SemanticResult]) -> String {
    if results.is_empty() {
        return "No semantic matches found\n".into();
    }

    let mut output = format!("Semantic results ({}):\n", results.len());
    for r in results {
        output.push_str(&format!(
            "  [{:.3}] {} {} {}:{}\n",
            r.score, r.entry.kind, r.entry.text, r.entry.file, r.entry.line
        ));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_works() {
        let tokens = tokenize("Hello, World! This is a test.");
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
        assert!(tokens.contains(&"test".to_string()));
    }

    #[test]
    fn normalize_works() {
        let v = normalize(&vec![3.0, 4.0]);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.001);
    }

    #[test]
    fn cosine_similarity_similar() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);
    }

    #[test]
    fn cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!((cosine_similarity(&a, &b)).abs() < 0.001);
    }

    #[test]
    fn vector_store_build_and_search() {
        let index = CodeIndex {
            version: "1.0".into(),
            project: crate::index::ProjectInfo {
                name: "test".into(),
                version: "0.1.0".into(),
                description: "Test".into(),
            },
            files: vec![],
            symbols: vec![
                crate::index::Symbol {
                    kind: "fn".into(),
                    name: "get_config".into(),
                    file: "src/config.rs".into(),
                    line: 10,
                    doc: Some("Get the configuration".into()),
                    signature: None,
                    module: None,
                },
                crate::index::Symbol {
                    kind: "struct".into(),
                    name: "Database".into(),
                    file: "src/db.rs".into(),
                    line: 5,
                    doc: Some("Database connection".into()),
                    signature: None,
                    module: None,
                },
            ],
        };

        let mut store = VectorStore::new();
        store.build_from_index(&index);

        let results = store.search("config", 5);
        assert!(!results.is_empty());
        assert!(results[0].entry.text.contains("config"));
    }
}
