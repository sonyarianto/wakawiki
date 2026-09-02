use crate::config::Config;
use serde::{Deserialize, Serialize};

const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 500;

pub async fn retry_request(
    _client: &reqwest::Client,
    request_builder: impl Fn() -> reqwest::RequestBuilder,
) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
    let mut last_err: Option<Box<dyn std::error::Error>> = None;
    for attempt in 0..MAX_RETRIES {
        let resp = request_builder().send().await;
        match resp {
            Ok(r) if r.status().is_success() || r.status().is_client_error() => return Ok(r),
            Ok(r) if r.status().as_u16() == 429 || r.status().is_server_error() => {
                let backoff = INITIAL_BACKOFF_MS * 2u64.pow(attempt);
                eprintln!(
                    "HTTP {} — retrying in {}ms (attempt {}/{})",
                    r.status(),
                    backoff,
                    attempt + 1,
                    MAX_RETRIES
                );
                let _ = r.text().await;
                tokio::time::sleep(std::time::Duration::from_millis(backoff)).await;
                last_err = None;
            }
            Ok(r) => {
                let status = r.status();
                let body = r.text().await.unwrap_or_default();
                return Err(format!("HTTP {status}: {body}").into());
            }
            Err(e) => {
                let backoff = INITIAL_BACKOFF_MS * 2u64.pow(attempt);
                eprintln!(
                    "Request failed: {e} — retrying in {}ms (attempt {}/{})",
                    backoff,
                    attempt + 1,
                    MAX_RETRIES
                );
                tokio::time::sleep(std::time::Duration::from_millis(backoff)).await;
                last_err = Some(Box::new(e));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| "all retries exhausted".into()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub message: ChatMessage,
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

pub enum LlmProvider {
    OpenAi(openai::OpenAiProvider),
    Anthropic(anthropic::AnthropicProvider),
    Opencode(opencode::OpencodeProvider),
}

impl LlmProvider {
    pub async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDef],
    ) -> Result<ChatResponse, Box<dyn std::error::Error>> {
        match self {
            LlmProvider::OpenAi(p) => p.chat(messages, tools).await,
            LlmProvider::Anthropic(p) => p.chat(messages, tools).await,
            LlmProvider::Opencode(p) => p.chat(messages, tools).await,
        }
    }
}

pub fn create(cfg: &Config) -> LlmProvider {
    match cfg.provider.as_str() {
        "anthropic" => LlmProvider::Anthropic(anthropic::AnthropicProvider::new(cfg)),
        "opencode" => LlmProvider::Opencode(opencode::OpencodeProvider::new(cfg)),
        "deepseek" => LlmProvider::OpenAi(openai::OpenAiProvider::with_base_url(
            cfg,
            "https://api.deepseek.com/v1",
        )),
        "openrouter" => LlmProvider::OpenAi(openai::OpenAiProvider::with_base_url(
            cfg,
            &cfg.base_url
                .clone()
                .unwrap_or_else(|| "https://openrouter.ai/api/v1".into()),
        )),
        _ => LlmProvider::OpenAi(openai::OpenAiProvider::new(cfg)),
    }
}

pub mod anthropic;
pub mod openai;
pub mod opencode;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn make_config(provider: &str) -> Config {
        Config {
            provider: provider.into(),
            api_key: "sk-test".into(),
            model: "test-model".into(),
            base_url: None,
        }
    }

    #[test]
    fn create_dispatches_openai() {
        let cfg = make_config("openai");
        let p = create(&cfg);
        assert!(matches!(p, LlmProvider::OpenAi(_)));
    }

    #[test]
    fn create_dispatches_anthropic() {
        let cfg = make_config("anthropic");
        let p = create(&cfg);
        assert!(matches!(p, LlmProvider::Anthropic(_)));
    }

    #[test]
    fn create_dispatches_opencode() {
        let cfg = make_config("opencode");
        let p = create(&cfg);
        assert!(matches!(p, LlmProvider::Opencode(_)));
    }

    #[test]
    fn create_dispatches_deepseek_as_openai() {
        let cfg = make_config("deepseek");
        let p = create(&cfg);
        assert!(matches!(p, LlmProvider::OpenAi(_)));
    }

    #[test]
    fn create_dispatches_openrouter_as_openai() {
        let cfg = make_config("openrouter");
        let p = create(&cfg);
        assert!(matches!(p, LlmProvider::OpenAi(_)));
    }

    #[test]
    fn create_unknown_provider_falls_back_to_openai() {
        let cfg = make_config("typo_provider");
        let p = create(&cfg);
        assert!(matches!(p, LlmProvider::OpenAi(_)));
    }
}
