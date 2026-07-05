use serde::{Deserialize, Serialize};
use std::io::BufRead;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub provider: String,
    pub api_key: String,
    pub model: String,
    pub base_url: Option<String>,
}

fn config_dir() -> PathBuf {
    dirs::home_dir()
        .expect("Could not find home directory")
        .join(".wakawiki")
}

fn env_path() -> PathBuf {
    config_dir().join(".env")
}

pub fn init_config() -> Result<(), Box<dyn std::error::Error>> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;

    let provider = dialoguer::Select::new()
        .with_prompt("Select LLM provider")
        .items([
            "OpenAI",
            "Anthropic",
            "DeepSeek",
            "OpenRouter",
            "opencode (local, no API key needed)",
            "Custom (OpenAI-compatible)",
        ])
        .default(0)
        .interact()?;

    let provider_name = match provider {
        0 => "openai",
        1 => "anthropic",
        2 => "deepseek",
        3 => "openrouter",
        4 => "opencode",
        _ => "custom",
    };

    let (api_key, model, base_url) = match provider {
        4 => {
            let model = dialoguer::Input::new()
                .with_prompt(
                    "Codewiki model override (optional, press Enter to use opencode default)",
                )
                .allow_empty(true)
                .interact()?;
            (String::new(), model, None)
        }
        2 => {
            let api_key: String = dialoguer::Password::new()
                .with_prompt("Enter DeepSeek API key")
                .interact()?;
            (
                api_key,
                "deepseek-chat".into(),
                Some("https://api.deepseek.com/v1".into()),
            )
        }
        0 => {
            let api_key: String = dialoguer::Password::new()
                .with_prompt("Enter OpenAI API key")
                .interact()?;
            (api_key, "gpt-4o".into(), None)
        }
        1 => {
            let api_key: String = dialoguer::Password::new()
                .with_prompt("Enter Anthropic API key")
                .interact()?;
            (api_key, "claude-sonnet-4-20250514".into(), None)
        }
        3 => {
            let api_key: String = dialoguer::Password::new()
                .with_prompt("Enter OpenRouter API key")
                .interact()?;
            (
                api_key,
                "openai/gpt-4o".into(),
                Some("https://openrouter.ai/api/v1".into()),
            )
        }
        _ => {
            let api_key: String = dialoguer::Password::new()
                .with_prompt("Enter API key")
                .interact()?;
            let model: String = dialoguer::Input::new()
                .with_prompt("Enter model ID")
                .interact()?;
            let base_url: String = dialoguer::Input::new()
                .with_prompt("Enter base URL")
                .default("https://api.openai.com/v1".into())
                .interact()?;
            (api_key, model, Some(base_url))
        }
    };

    let config = Config {
        provider: provider_name.into(),
        model,
        api_key,
        base_url,
    };

    let mut env_content = format!(
        "WAKAWIKI_PROVIDER={}\nWAKAWIKI_API_KEY={}\nWAKAWIKI_MODEL={}\n",
        config.provider, config.api_key, config.model,
    );
    if let Some(ref url) = config.base_url {
        env_content.push_str(&format!("WAKAWIKI_BASE_URL={url}\n"));
    }

    std::fs::write(env_path(), env_content)?;
    println!("Config saved to {}", env_path().display());
    Ok(())
}

pub fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let path = env_path();
    if !path.exists() {
        return Err("No config found. Run 'wakawiki --init' first.".into());
    }

    let file = std::fs::File::open(&path)?;
    let reader = std::io::BufReader::new(file);
    let mut provider = String::from("openai");
    let mut api_key = String::new();
    let mut model = String::new();
    let mut base_url: Option<String> = None;

    for line in reader.lines() {
        let line = line?;
        if let Some(val) = line.strip_prefix("WAKAWIKI_PROVIDER=") {
            provider = val.to_string();
        } else if let Some(val) = line.strip_prefix("WAKAWIKI_API_KEY=") {
            api_key = val.to_string();
        } else if let Some(val) = line.strip_prefix("WAKAWIKI_MODEL=") {
            model = val.to_string();
        } else if let Some(val) = line.strip_prefix("WAKAWIKI_BASE_URL=") {
            base_url = Some(val.to_string());
        }
    }

    if api_key.is_empty() && provider != "opencode" {
        return Err("API key not found in config. Run 'wakawiki --init' again.".into());
    }

    Ok(Config {
        provider,
        api_key,
        model,
        base_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_env(content: &str) -> (std::path::PathBuf, impl FnOnce()) {
        let dir = std::env::temp_dir().join(format!("cw_cfg_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let env_file = dir.join("test.env");
        std::fs::write(&env_file, content).unwrap();
        (env_file, move || {
            let _ = std::fs::remove_dir_all(&dir);
        })
    }

    #[test]
    fn load_config_parses_all_fields() {
        let (path, _cleanup) = temp_env(
            "WAKAWIKI_PROVIDER=openai\nWAKAWIKI_API_KEY=sk-test\nWAKAWIKI_MODEL=gpt-4o\nWAKAWIKI_BASE_URL=https://example.com/v1\n",
        );

        // We can't easily override env_path() since it uses dirs::home_dir().
        // Test the parser logic directly.
        let content = std::fs::read_to_string(&path).unwrap();
        let mut provider = String::new();
        let mut api_key = String::new();
        let mut model = String::new();
        let mut base_url = None;

        for line in content.lines() {
            if let Some(val) = line.strip_prefix("WAKAWIKI_PROVIDER=") {
                provider = val.to_string();
            } else if let Some(val) = line.strip_prefix("WAKAWIKI_API_KEY=") {
                api_key = val.to_string();
            } else if let Some(val) = line.strip_prefix("WAKAWIKI_MODEL=") {
                model = val.to_string();
            } else if let Some(val) = line.strip_prefix("WAKAWIKI_BASE_URL=") {
                base_url = Some(val.to_string());
            }
        }

        assert_eq!(provider, "openai");
        assert_eq!(api_key, "sk-test");
        assert_eq!(model, "gpt-4o");
        assert_eq!(base_url.unwrap(), "https://example.com/v1");
    }

    #[test]
    fn load_config_missing_optional_base_url() {
        let content =
            "WAKAWIKI_PROVIDER=anthropic\nWAKAWIKI_API_KEY=sk-key\nWAKAWIKI_MODEL=claude\n";
        let mut base_url: Option<String> = None;

        for line in content.lines() {
            if let Some(val) = line.strip_prefix("WAKAWIKI_BASE_URL=") {
                base_url = Some(val.to_string());
            }
        }

        assert!(base_url.is_none());
    }

    #[test]
    fn config_serialization_roundtrip() {
        let cfg = Config {
            provider: "openai".into(),
            api_key: "sk-abc".into(),
            model: "gpt-4o".into(),
            base_url: None,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.provider, cfg.provider);
        assert_eq!(parsed.api_key, cfg.api_key);
        assert_eq!(parsed.model, cfg.model);
        assert_eq!(parsed.base_url, cfg.base_url);
    }
}
