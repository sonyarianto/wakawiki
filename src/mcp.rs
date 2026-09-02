use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};

use crate::index::{self, CodeIndex};
use crate::tools;
use crate::vector::VectorStore;

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct ToolInfo {
    name: String,
    description: String,
    #[serde(rename = "inputSchema")]
    input_schema: serde_json::Value,
}

pub fn run_server(project_dir: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let index = match index::load_index(project_dir) {
        Ok(idx) => idx,
        Err(_) => {
            eprintln!("No index found. Run 'wakawiki --index' first.");
            std::process::exit(1);
        }
    };

    let vector_store = VectorStore::load(project_dir).ok();

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    let mut buffer = String::new();

    loop {
        buffer.clear();
        match reader.read_line(&mut buffer) {
            Ok(0) => break,
            Ok(_) => {
                let line = buffer.trim();
                if line.is_empty() {
                    continue;
                }

                let request: JsonRpcRequest = match serde_json::from_str(line) {
                    Ok(r) => r,
                    Err(_) => continue,
                };

                let response = handle_request(&request, &index, vector_store.as_ref());
                let json = serde_json::to_string(&response).unwrap_or_default();
                writeln!(writer, "{json}")?;
                writer.flush()?;
            }
            Err(e) => {
                eprintln!("Read error: {e}");
                break;
            }
        }
    }

    Ok(())
}

fn handle_request(
    request: &JsonRpcRequest,
    index: &CodeIndex,
    vector_store: Option<&VectorStore>,
) -> JsonRpcResponse {
    match request.method.as_str() {
        "initialize" => handle_initialize(request),
        "notifications/initialized" => {
            // Client notification, no response needed per MCP spec
            // But we still need to return something for JSON-RPC
            JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: None,
                result: Some(serde_json::json!({})),
                error: None,
            }
        }
        "tools/list" => handle_tools_list(request, vector_store.is_some()),
        "tools/call" => handle_tools_call(request, index, vector_store),
        "ping" => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: request.id.clone(),
            result: Some(serde_json::json!({})),
            error: None,
        },
        _ => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: request.id.clone(),
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: format!("Method not found: {}", request.method),
                data: None,
            }),
        },
    }
}

fn handle_initialize(request: &JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id: request.id.clone(),
        result: Some(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {
                    "listChanged": false
                }
            },
            "serverInfo": {
                "name": "wakawiki",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),
        error: None,
    }
}

fn handle_tools_list(request: &JsonRpcRequest, has_embeddings: bool) -> JsonRpcResponse {
    let mut tools = vec![
        ToolInfo {
            name: "query_symbols".into(),
            description:
                "Search for symbols (functions, structs, enums, etc.) by name, kind, or file path"
                    .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Search pattern (case-insensitive)"
                    }
                },
                "required": ["pattern"]
            }),
        },
        ToolInfo {
            name: "get_symbol".into(),
            description: "Get detailed information about a specific symbol by name".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Symbol name to look up"
                    },
                    "file": {
                        "type": "string",
                        "description": "Optional file path to disambiguate"
                    }
                },
                "required": ["name"]
            }),
        },
        ToolInfo {
            name: "list_files".into(),
            description: "List all indexed files with their language and size".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "language": {
                        "type": "string",
                        "description": "Optional filter by language (e.g., 'rust', 'javascript')"
                    }
                }
            }),
        },
        ToolInfo {
            name: "get_file_info".into(),
            description: "Get detailed information about a specific file including its symbols"
                .into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path to look up"
                    }
                },
                "required": ["path"]
            }),
        },
        ToolInfo {
            name: "get_project_info".into(),
            description: "Get project metadata (name, version, description)".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
    ];

    if has_embeddings {
        tools.push(ToolInfo {
            name: "semantic_search".into(),
            description: "Semantic search using vector embeddings (requires --embed)".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Natural language query for semantic search"
                    },
                    "top_k": {
                        "type": "integer",
                        "description": "Number of results to return (default: 10)"
                    }
                },
                "required": ["query"]
            }),
        });
    }

    JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id: request.id.clone(),
        result: Some(serde_json::json!({
            "tools": tools
        })),
        error: None,
    }
}

fn handle_tools_call(
    request: &JsonRpcRequest,
    index: &CodeIndex,
    vector_store: Option<&VectorStore>,
) -> JsonRpcResponse {
    let tool_name = request.params.get("name").and_then(|v| v.as_str());
    let arguments = request.params.get("arguments");

    let result = match tool_name {
        Some("query_symbols") => {
            let pattern = arguments
                .and_then(|a| a.get("pattern"))
                .and_then(|p| p.as_str())
                .unwrap_or("");
            let text = tools::index::query_symbols(index, pattern);
            Ok(serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": text
                }]
            }))
        }
        Some("get_symbol") => {
            let name = arguments
                .and_then(|a| a.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("");
            let file = arguments
                .and_then(|a| a.get("file"))
                .and_then(|f| f.as_str());

            match tools::index::get_symbol(index, name, file) {
                Ok(text) => Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": text
                    }]
                })),
                Err(e) => Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": e
                    }],
                    "isError": true
                })),
            }
        }
        Some("list_files") => {
            let language = arguments
                .and_then(|a| a.get("language"))
                .and_then(|l| l.as_str());

            let text = tools::index::list_files_text(index, language);

            Ok(serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": text
                }]
            }))
        }
        Some("get_file_info") => {
            let path = arguments
                .and_then(|a| a.get("path"))
                .and_then(|p| p.as_str())
                .unwrap_or("");

            match tools::index::get_file_info(index, path) {
                Ok(text) => Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": text
                    }]
                })),
                Err(e) => Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": e
                    }],
                    "isError": true
                })),
            }
        }
        Some("get_project_info") => {
            let text = tools::index::get_project_info(index);
            Ok(serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": text
                }]
            }))
        }
        Some("semantic_search") => match vector_store {
            Some(store) => {
                let query = arguments
                    .and_then(|a| a.get("query"))
                    .and_then(|q| q.as_str())
                    .unwrap_or("");
                let top_k = arguments
                    .and_then(|a| a.get("top_k"))
                    .and_then(|k| k.as_u64())
                    .unwrap_or(10) as usize;

                let text = tools::index::semantic_search_text(store, query, top_k);

                Ok(serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": text
                    }]
                }))
            }
            None => Ok(serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": "Semantic search not available. Run 'wakawiki --embed' first.".to_string()
                }],
                "isError": true
            })),
        },
        Some(name) => Err(JsonRpcError {
            code: -32602,
            message: format!("Unknown tool: {name}"),
            data: None,
        }),
        None => Err(JsonRpcError {
            code: -32602,
            message: "No tool name provided".into(),
            data: None,
        }),
    };

    match result {
        Ok(content) => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: request.id.clone(),
            result: Some(content),
            error: None,
        },
        Err(error) => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: request.id.clone(),
            result: None,
            error: Some(error),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{FileEntry, ProjectInfo, Symbol};

    fn test_index() -> CodeIndex {
        CodeIndex {
            version: "1.0".into(),
            project: ProjectInfo {
                name: "test".into(),
                version: "0.1.0".into(),
                description: "A test project".into(),
            },
            files: vec![FileEntry {
                path: "src/main.rs".into(),
                size: 1024,
                hash: "abc123".into(),
                language: "rust".into(),
            }],
            symbols: vec![Symbol {
                kind: "fn".into(),
                name: "main".into(),
                file: "src/main.rs".into(),
                line: 1,
                doc: Some("Entry point".into()),
                signature: Some("fn main()".into()),
                module: None,
            }],
        }
    }

    #[test]
    fn handle_initialize_works() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(1)),
            method: "initialize".into(),
            params: serde_json::json!({}),
        };

        let response = handle_initialize(&request);
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn handle_tools_list_works() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(1)),
            method: "tools/list".into(),
            params: serde_json::json!({}),
        };

        let response = handle_tools_list(&request, false);
        assert!(response.result.is_some());
        let result = response.result.unwrap();
        let tools = result.get("tools").unwrap().as_array().unwrap();
        assert_eq!(tools.len(), 5);
    }

    #[test]
    fn handle_tools_list_with_embeddings() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(1)),
            method: "tools/list".into(),
            params: serde_json::json!({}),
        };

        let response = handle_tools_list(&request, true);
        assert!(response.result.is_some());
        let result = response.result.unwrap();
        let tools = result.get("tools").unwrap().as_array().unwrap();
        assert_eq!(tools.len(), 6);
    }

    #[test]
    fn handle_tools_call_query_symbols() {
        let index = test_index();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(1)),
            method: "tools/call".into(),
            params: serde_json::json!({
                "name": "query_symbols",
                "arguments": {"pattern": "main"}
            }),
        };

        let response = handle_tools_call(&request, &index, None);
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn handle_unknown_method() {
        let index = test_index();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(1)),
            method: "unknown".into(),
            params: serde_json::json!({}),
        };

        let response = handle_request(&request, &index, None);
        assert!(response.error.is_some());
    }
}
