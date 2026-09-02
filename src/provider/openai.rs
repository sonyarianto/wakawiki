use std::time::Duration;

use crate::config::Config;
use serde_json::Value;

use super::{ChatMessage, ChatResponse, ToolCall, ToolDef};

pub struct OpenAiProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenAiProvider {
    pub fn new(cfg: &Config) -> Self {
        let base_url = cfg
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.openai.com/v1".into());
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .expect("Failed to build HTTP client"),
            api_key: cfg.api_key.clone(),
            model: cfg.model.clone(),
            base_url,
        }
    }

    pub fn with_base_url(cfg: &Config, base_url: &str) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .expect("Failed to build HTTP client"),
            api_key: cfg.api_key.clone(),
            model: cfg.model.clone(),
            base_url: base_url.into(),
        }
    }

    pub async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDef],
    ) -> Result<ChatResponse, Box<dyn std::error::Error>> {
        let api_messages: Vec<Value> = messages
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
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    }
                })
            })
            .collect();

        let mut body = serde_json::json!({
            "model": self.model,
            "messages": api_messages,
            "max_tokens": 4096,
        });

        if !api_tools.is_empty() {
            body["tools"] = serde_json::json!(api_tools);
            body["tool_choice"] = serde_json::json!("auto");
        }

        let resp = super::retry_request(|| {
            self.client
                .post(format!("{}/chat/completions", self.base_url))
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&body)
        })
        .await?;

        let status = resp.status();
        let body_text = resp.text().await?;

        if !status.is_success() {
            return Err(format!("OpenAI API error ({}): {}", status, body_text).into());
        }

        let json: Value = serde_json::from_str(&body_text)?;
        let choice = json["choices"]
            .as_array()
            .and_then(|arr| arr.first())
            .ok_or("OpenAI API returned empty choices")?;

        let content = choice["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let tool_calls: Option<Vec<ToolCall>> =
            choice["message"]["tool_calls"].as_array().map(|tc_arr| {
                tc_arr
                    .iter()
                    .map(|tc| ToolCall {
                        id: tc["id"].as_str().unwrap_or("").to_string(),
                        name: tc["function"]["name"].as_str().unwrap_or("").to_string(),
                        arguments: tc["function"]["arguments"]
                            .as_str()
                            .unwrap_or("{}")
                            .to_string(),
                    })
                    .collect()
            });

        Ok(ChatResponse {
            message: ChatMessage {
                role: "assistant".into(),
                content,
            },
            tool_calls,
        })
    }
}
