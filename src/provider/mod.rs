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
            Ok(r) if r.status().is_success() => return Ok(r),
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
            Ok(r) if r.status().is_client_error() => return Ok(r),
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

    #[tokio::test]
    async fn retry_request_retries_on_429_then_succeeds() {
        use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        // First request returns 429, second succeeds — verify retry fires
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(429).set_body_string("rate limited"))
            .up_to_n_times(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server)
            .await;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();

        let result = retry_request(&client, || client.get(format!("{}/test", server.uri()))).await;

        assert!(
            result.is_ok(),
            "expected success after 429 retry, got: {result:?}"
        );
        assert_eq!(result.unwrap().status().as_u16(), 200);

        // Verify exactly 2 requests were made
        let received = server.received_requests().await.unwrap();
        assert_eq!(received.len(), 2, "expected 2 requests (1 retry on 429)");
    }

    #[tokio::test]
    async fn retry_request_does_not_retry_on_400() {
        use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(400).set_body_string("bad request"))
            .mount(&server)
            .await;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();

        let result = retry_request(&client, || client.get(format!("{}/test", server.uri()))).await;

        assert!(result.is_ok(), "400 should return Ok without retry");
        assert_eq!(result.unwrap().status().as_u16(), 400);

        let received = server.received_requests().await.unwrap();
        assert_eq!(received.len(), 1, "400 should not trigger retry");
    }

    #[tokio::test]
    async fn retry_request_retries_on_500_then_succeeds() {
        use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500).set_body_string("server error"))
            .up_to_n_times(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server)
            .await;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap();

        let result = retry_request(&client, || client.get(format!("{}/test", server.uri()))).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().status().as_u16(), 200);

        let received = server.received_requests().await.unwrap();
        assert_eq!(received.len(), 2, "expected retry on 500");
    }
}
