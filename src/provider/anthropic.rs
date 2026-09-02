use std::time::Duration;

use crate::config::Config;
use serde_json::Value;

use super::{ChatMessage, ChatResponse, ToolCall, ToolDef};

pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl AnthropicProvider {
    pub fn new(cfg: &Config) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .expect("Failed to build HTTP client"),
            api_key: cfg.api_key.clone(),
            model: cfg.model.clone(),
        }
    }

    pub async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDef],
    ) -> Result<ChatResponse, Box<dyn std::error::Error>> {
        let (system_prompts, conversation): (Vec<&ChatMessage>, Vec<&ChatMessage>) =
            messages.iter().partition(|m| m.role == "system");

        let system_content = system_prompts
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        let api_messages: Vec<Value> = conversation
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": m.role,
                    "content": m.content,
                })
            })
            .collect();

        let api_tools: Vec<Value> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters,
                })
            })
            .collect();

        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": api_messages,
        });

        if !system_content.is_empty() {
            body["system"] = serde_json::json!(system_content);
        }

        if !api_tools.is_empty() {
            body["tools"] = serde_json::json!(api_tools);
        }

        let resp = super::retry_request(&self.client, || {
            self.client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("Content-Type", "application/json")
                .json(&body)
        })
        .await?;

        let status = resp.status();
        let body_text = resp.text().await?;

        if !status.is_success() {
            return Err(format!("Anthropic API error ({}): {}", status, body_text).into());
        }

        let json: Value = serde_json::from_str(&body_text)?;

        let mut text_content = String::new();
        let mut tool_use_blocks: Vec<ToolCall> = Vec::new();

        if let Some(content_arr) = json["content"].as_array() {
            for block in content_arr {
                match block["type"].as_str() {
                    Some("text") => {
                        if let Some(t) = block["text"].as_str() {
                            text_content.push_str(t);
                        }
                    }
                    Some("tool_use") => {
                        tool_use_blocks.push(ToolCall {
                            id: block["id"].as_str().unwrap_or("").to_string(),
                            name: block["name"].as_str().unwrap_or("").to_string(),
                            arguments: block["input"].to_string(),
                        });
                    }
                    _ => {}
                }
            }
        }

        Ok(ChatResponse {
            message: ChatMessage {
                role: "assistant".into(),
                content: text_content,
            },
            tool_calls: if tool_use_blocks.is_empty() {
                None
            } else {
                Some(tool_use_blocks)
            },
        })
    }
}
