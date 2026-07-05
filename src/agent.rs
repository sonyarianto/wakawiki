use std::collections::HashMap;
use std::path::Path;

use crate::config::Config;
use crate::output::{self, WikiMeta};
use crate::prompts;
use crate::provider::{ChatMessage, LlmProvider, ToolCall, ToolDef};
use crate::scanner;

fn tool_definitions() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "list_files".into(),
            description: "List files and directories at a given path. Pass an empty string to list the root directory.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to list. Empty string for root."
                    }
                },
                "required": ["path"]
            }),
        },
        ToolDef {
            name: "read_file".into(),
            description: "Read the full contents of a file.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the file to read."
                    }
                },
                "required": ["path"]
            }),
        },
        ToolDef {
            name: "search".into(),
            description: "Search the codebase for a pattern. Returns matching lines with file paths and line numbers. Use this to find definitions, usages, or patterns.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Text or substring to search for."
                    }
                },
                "required": ["pattern"]
            }),
        },
        ToolDef {
            name: "write_doc".into(),
            description: "Write a documentation file into the wakawiki/ output directory. Use this to create or update documentation. Do NOT write outside wakawiki/.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path within wakawiki/ to write to (e.g. 'index.md', 'architecture.md')."
                    },
                    "content": {
                        "type": "string",
                        "description": "Markdown content for the documentation file."
                    }
                },
                "required": ["path", "content"]
            }),
        },
    ]
}

fn execute_tool(
    tool: &ToolCall,
    project_dir: &Path,
    wakawiki_dir: &Path,
    wiki_meta: &mut WikiMeta,
) -> String {
    match tool.name.as_str() {
        "list_files" => {
            let args: HashMap<String, String> =
                serde_json::from_str(&tool.arguments).unwrap_or_default();
            let subpath = args.get("path").map(|s| s.as_str()).unwrap_or("");
            let dir_to_list = if subpath.is_empty() {
                project_dir.to_path_buf()
            } else {
                project_dir.join(subpath.trim_start_matches('/'))
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
        "read_file" => {
            let args: HashMap<String, String> =
                serde_json::from_str(&tool.arguments).unwrap_or_default();
            let filepath = args.get("path").map(|s| s.as_str()).unwrap_or("");

            if filepath.is_empty() {
                return "Error: no path provided".into();
            }

            let full_path = project_dir.join(filepath.trim_start_matches('/'));
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
        "search" => {
            let args: HashMap<String, String> =
                serde_json::from_str(&tool.arguments).unwrap_or_default();
            let pattern = args.get("pattern").map(|s| s.as_str()).unwrap_or("");

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
        "write_doc" => {
            let args: HashMap<String, String> =
                serde_json::from_str(&tool.arguments).unwrap_or_default();
            let path = args.get("path").map(|s| s.as_str()).unwrap_or("");
            let content = args.get("content").map(|s| s.as_str()).unwrap_or("");

            if path.is_empty() {
                return "Error: no path provided".into();
            }

            let full_path = output::write_doc(wakawiki_dir, path, content);
            if let Ok(hash) = scanner::compute_file_hash(&full_path) {
                wiki_meta.file_hashes.insert(path.to_string(), hash);
            }
            format!(
                "Documentation written: wakawiki/{} ({} bytes)",
                path,
                content.len()
            )
        }
        _ => format!("Unknown tool: {}", tool.name),
    }
}

pub async fn run_interactive(
    project_dir: &Path,
    provider: &LlmProvider,
    _cfg: &Config,
    initial_prompt: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let wakawiki_dir = project_dir.join("wakawiki");
    std::fs::create_dir_all(&wakawiki_dir)?;

    let mut wiki_meta = output::load_wiki_meta(&wakawiki_dir);
    let tools = tool_definitions();

    let mut messages: Vec<ChatMessage> = vec![
        ChatMessage {
            role: "system".into(),
            content: prompts::SYSTEM_PROMPT.into(),
        },
        ChatMessage {
            role: "user".into(),
            content: prompts::initial_prompt(
                initial_prompt
                    .unwrap_or("Please generate comprehensive documentation for this codebase."),
            ),
        },
    ];

    let mut total_tool_calls = 0;
    let max_tool_calls = 100;

    loop {
        let resp = provider.chat(&messages, &tools).await?;

        if let Some(tool_calls) = resp.tool_calls {
            if tool_calls.is_empty() {
                if !resp.message.content.is_empty() {
                    println!("\n{}", resp.message.content);
                }
                break;
            }

            messages.push(ChatMessage {
                role: "assistant".into(),
                content: if resp.message.content.is_empty() {
                    String::new()
                } else {
                    resp.message.content.clone()
                },
            });

            for tc in &tool_calls {
                println!("  -> {}", tc.name);

                let result = execute_tool(tc, project_dir, &wakawiki_dir, &mut wiki_meta);

                messages.push(ChatMessage {
                    role: "tool".into(),
                    content: result,
                });

                total_tool_calls += 1;

                if total_tool_calls >= max_tool_calls {
                    println!(
                        "\nReached maximum tool calls ({}). Stopping.",
                        max_tool_calls
                    );
                    break;
                }
            }

            messages.push(ChatMessage {
                role: "user".into(),
                content: prompts::tool_response_prompt().into(),
            });
        } else {
            println!("\n{}", resp.message.content);

            if resp.message.content.to_uppercase().contains("DONE") {
                break;
            }

            let read_line = || -> Option<String> {
                use std::io::Write;
                print!("\n> ");
                let _ = std::io::stdout().flush();
                let mut input = String::new();
                match std::io::stdin().read_line(&mut input) {
                    Ok(0) => None,
                    Ok(_) => Some(input.trim().to_string()),
                    Err(_) => None,
                }
            };

            while let Some(user_input) = read_line() {
                if user_input.is_empty() {
                    continue;
                }
                if user_input.eq_ignore_ascii_case("quit")
                    || user_input.eq_ignore_ascii_case("exit")
                    || user_input.eq_ignore_ascii_case("done")
                {
                    return Ok(());
                }
                messages.push(ChatMessage {
                    role: "user".into(),
                    content: user_input,
                });
                break;
            }
        }
    }

    output::save_wiki_meta(&wakawiki_dir, &wiki_meta);

    let _ = output::append_agents_reference(project_dir);

    println!("\nDocumentation written to wakawiki/");
    Ok(())
}

pub async fn run_oneshot(
    project_dir: &Path,
    provider: &LlmProvider,
    _cfg: &Config,
    prompt: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let wakawiki_dir = project_dir.join("wakawiki");
    std::fs::create_dir_all(&wakawiki_dir)?;

    let mut wiki_meta = output::load_wiki_meta(&wakawiki_dir);
    let tools = tool_definitions();

    let mut messages: Vec<ChatMessage> = vec![
        ChatMessage {
            role: "system".into(),
            content: prompts::SYSTEM_PROMPT.into(),
        },
        ChatMessage {
            role: "user".into(),
            content: prompts::initial_prompt(prompt),
        },
    ];

    let mut total_tool_calls = 0;
    let max_tool_calls = 100;
    let mut final_output = String::new();

    loop {
        let resp = provider.chat(&messages, &tools).await?;

        if let Some(tool_calls) = resp.tool_calls {
            if tool_calls.is_empty() {
                final_output.push_str(&resp.message.content);
                break;
            }

            messages.push(ChatMessage {
                role: "assistant".into(),
                content: resp.message.content,
            });

            for tc in &tool_calls {
                let result = execute_tool(tc, project_dir, &wakawiki_dir, &mut wiki_meta);
                messages.push(ChatMessage {
                    role: "tool".into(),
                    content: result,
                });
                total_tool_calls += 1;
                if total_tool_calls >= max_tool_calls {
                    break;
                }
            }

            messages.push(ChatMessage {
                role: "user".into(),
                content: prompts::tool_response_prompt().into(),
            });
        } else {
            final_output.push_str(&resp.message.content);
            break;
        }
    }

    Ok(final_output)
}

pub async fn update_docs(
    project_dir: &Path,
    wakawiki_dir: &Path,
    wiki_meta: &mut WikiMeta,
    provider: &LlmProvider,
    _cfg: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let tools = tool_definitions();

    let mut messages: Vec<ChatMessage> = vec![
        ChatMessage {
            role: "system".into(),
            content: prompts::SYSTEM_PROMPT.into(),
        },
        ChatMessage {
            role: "user".into(),
            content: "The wakawiki/ directory already has existing documentation. Please review the codebase for any changes since the docs were last generated, and update the documentation files with write_doc to reflect the current state of the codebase. Focus on what changed.".into(),
        },
    ];

    let mut total_tool_calls = 0;
    let max_tool_calls = 100;

    loop {
        let resp = provider.chat(&messages, &tools).await?;

        if let Some(tool_calls) = resp.tool_calls {
            if tool_calls.is_empty() {
                if !resp.message.content.is_empty() {
                    println!("\n{}", resp.message.content);
                }
                break;
            }

            messages.push(ChatMessage {
                role: "assistant".into(),
                content: resp.message.content,
            });

            for tc in &tool_calls {
                let result = execute_tool(tc, project_dir, wakawiki_dir, wiki_meta);
                messages.push(ChatMessage {
                    role: "tool".into(),
                    content: result,
                });
                total_tool_calls += 1;
                if total_tool_calls >= max_tool_calls {
                    break;
                }
            }

            messages.push(ChatMessage {
                role: "user".into(),
                content: prompts::tool_response_prompt().into(),
            });
        } else {
            println!("\n{}", resp.message.content);
            break;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn temp_project() -> (std::path::PathBuf, impl FnOnce()) {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("cw_agent_{}_{}", std::process::id(), n));
        std::fs::create_dir_all(&dir).unwrap();
        let d = dir.clone();
        (dir, move || {
            let _ = std::fs::remove_dir_all(&d);
        })
    }

    fn make_tool_call(name: &str, args: &str) -> ToolCall {
        ToolCall {
            id: "call_1".into(),
            name: name.into(),
            arguments: args.into(),
        }
    }

    fn empty_meta() -> WikiMeta {
        WikiMeta {
            file_hashes: HashMap::new(),
        }
    }

    #[test]
    fn execute_list_files_shows_files() {
        let (proj, cleanup) = temp_project();
        std::fs::write(proj.join("a.rs"), "fn main() {}").unwrap();
        std::fs::write(proj.join("README.md"), "# Project").unwrap();

        let wakawiki = proj.join("wakawiki");
        std::fs::create_dir_all(&wakawiki).unwrap();

        let tc = make_tool_call("list_files", r#"{"path":""}"#);
        let result = execute_tool(&tc, &proj, &wakawiki, &mut empty_meta());
        assert!(result.contains("a.rs"));
        assert!(result.contains("README.md"));
        cleanup();
    }

    #[test]
    fn execute_list_files_subdirectory() {
        let (proj, cleanup) = temp_project();
        std::fs::create_dir_all(proj.join("sub")).unwrap();
        std::fs::write(proj.join("sub/b.rs"), "fn bar() {}").unwrap();

        let wakawiki = proj.join("wakawiki");
        std::fs::create_dir_all(&wakawiki).unwrap();

        let tc = make_tool_call("list_files", r#"{"path":"sub"}"#);
        let result = execute_tool(&tc, &proj, &wakawiki, &mut empty_meta());
        assert!(result.contains("b.rs"));
        cleanup();
    }

    #[test]
    fn execute_list_files_nonexistent_path() {
        let (proj, cleanup) = temp_project();
        let wakawiki = proj.join("wakawiki");
        std::fs::create_dir_all(&wakawiki).unwrap();

        let tc = make_tool_call("list_files", r#"{"path":"noexist"}"#);
        let result = execute_tool(&tc, &proj, &wakawiki, &mut empty_meta());
        assert!(result.contains("Error"));
        cleanup();
    }

    #[test]
    fn execute_read_file_returns_content() {
        let (proj, cleanup) = temp_project();
        std::fs::write(proj.join("hello.txt"), "hello world\n").unwrap();
        let wakawiki = proj.join("wakawiki");
        std::fs::create_dir_all(&wakawiki).unwrap();

        let tc = make_tool_call("read_file", r#"{"path":"hello.txt"}"#);
        let result = execute_tool(&tc, &proj, &wakawiki, &mut empty_meta());
        assert!(result.contains("hello world"));
        cleanup();
    }

    #[test]
    fn execute_read_file_empty_path() {
        let (proj, cleanup) = temp_project();
        let wakawiki = proj.join("wakawiki");
        std::fs::create_dir_all(&wakawiki).unwrap();

        let tc = make_tool_call("read_file", r#"{"path":""}"#);
        let result = execute_tool(&tc, &proj, &wakawiki, &mut empty_meta());
        assert!(result.contains("Error"));
        cleanup();
    }

    #[test]
    fn execute_search_finds_pattern() {
        let (proj, cleanup) = temp_project();
        std::fs::write(proj.join("src.rs"), "fn search_me() {\n    do_thing();\n}").unwrap();
        let wakawiki = proj.join("wakawiki");
        std::fs::create_dir_all(&wakawiki).unwrap();

        let tc = make_tool_call("search", r#"{"pattern":"search_me"}"#);
        let result = execute_tool(&tc, &proj, &wakawiki, &mut empty_meta());
        assert!(result.contains("search_me"));
        cleanup();
    }

    #[test]
    fn execute_search_no_match() {
        let (proj, cleanup) = temp_project();
        std::fs::write(proj.join("src.rs"), "fn x() {}").unwrap();
        let wakawiki = proj.join("wakawiki");
        std::fs::create_dir_all(&wakawiki).unwrap();

        let tc = make_tool_call("search", r#"{"pattern":"nonexistent"}"#);
        let result = execute_tool(&tc, &proj, &wakawiki, &mut empty_meta());
        assert!(result.contains("No matches"));
        cleanup();
    }

    #[test]
    fn execute_write_doc_creates_file() {
        let (proj, cleanup) = temp_project();
        let wakawiki = proj.join("wakawiki");
        std::fs::create_dir_all(&wakawiki).unwrap();

        let tc = make_tool_call(
            "write_doc",
            r##"{"path":"test.md","content":"# Test Doc"}"##,
        );
        let mut meta = empty_meta();
        let result = execute_tool(&tc, &proj, &wakawiki, &mut meta);
        assert!(result.contains("Documentation written"));
        assert!(wakawiki.join("test.md").exists());
        assert!(meta.file_hashes.contains_key("test.md"));
        cleanup();
    }

    #[test]
    fn execute_write_doc_empty_path() {
        let (proj, cleanup) = temp_project();
        let wakawiki = proj.join("wakawiki");
        std::fs::create_dir_all(&wakawiki).unwrap();

        let tc = make_tool_call("write_doc", r#"{"path":"","content":"x"}"#);
        let result = execute_tool(&tc, &proj, &wakawiki, &mut empty_meta());
        assert!(result.contains("Error"));
        cleanup();
    }

    #[test]
    fn execute_unknown_tool() {
        let (proj, cleanup) = temp_project();
        let wakawiki = proj.join("wakawiki");
        std::fs::create_dir_all(&wakawiki).unwrap();

        let tc = make_tool_call("nonexistent_tool", "{}");
        let result = execute_tool(&tc, &proj, &wakawiki, &mut empty_meta());
        assert!(result.contains("Unknown tool"));
        cleanup();
    }

    #[test]
    fn tool_definitions_include_all_four() {
        let defs = tool_definitions();
        assert_eq!(defs.len(), 4);
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"list_files"));
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"search"));
        assert!(names.contains(&"write_doc"));
    }
}
