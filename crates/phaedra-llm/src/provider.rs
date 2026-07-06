#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Ollama,
    OpenAI,
    Anthropic,
}

impl Provider {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "openai" => Provider::OpenAI,
            "anthropic" => Provider::Anthropic,
            _ => Provider::Ollama,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub provider: Provider,
    pub model: String,
}

impl LlmResponse {
    pub fn estimated_cost_usd(&self) -> Option<f64> {
        match (&self.provider, self.model.as_str()) {
            (Provider::OpenAI, m) if m.contains("gpt-4o-mini") => {
                Some(self.input_tokens as f64 * 0.15e-6 + self.output_tokens as f64 * 0.6e-6)
            }
            (Provider::OpenAI, m) if m.contains("gpt-4o") => {
                Some(self.input_tokens as f64 * 2.5e-6 + self.output_tokens as f64 * 10e-6)
            }
            (Provider::Anthropic, m) if m.contains("claude-3-5-sonnet") => {
                Some(self.input_tokens as f64 * 3e-6 + self.output_tokens as f64 * 15e-6)
            }
            (Provider::Anthropic, m) if m.contains("claude-3-haiku") => {
                Some(self.input_tokens as f64 * 0.25e-6 + self.output_tokens as f64 * 1.25e-6)
            }
            (Provider::Ollama, _) => Some(0.0),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_from_str_openai() {
        assert_eq!(Provider::from_str("openai"), Provider::OpenAI);
        assert_eq!(Provider::from_str("OpenAI"), Provider::OpenAI);
    }

    #[test]
    fn test_provider_from_str_anthropic() {
        assert_eq!(Provider::from_str("anthropic"), Provider::Anthropic);
        assert_eq!(Provider::from_str("ANTHROPIC"), Provider::Anthropic);
    }

    #[test]
    fn test_provider_from_str_unknown_defaults_ollama() {
        assert_eq!(Provider::from_str("unknown"), Provider::Ollama);
        assert_eq!(Provider::from_str(""), Provider::Ollama);
        assert_eq!(Provider::from_str("ollama"), Provider::Ollama);
    }

    #[test]
    fn test_ollama_cost_is_zero() {
        let r = LlmResponse {
            content: "x".into(),
            input_tokens: 100,
            output_tokens: 50,
            provider: Provider::Ollama,
            model: "llama3.2".into(),
        };
        assert_eq!(r.estimated_cost_usd(), Some(0.0));
    }

    #[test]
    fn test_openai_gpt4o_cost_positive() {
        let r = LlmResponse {
            content: "x".into(),
            input_tokens: 1000,
            output_tokens: 500,
            provider: Provider::OpenAI,
            model: "gpt-4o".into(),
        };
        assert!(r.estimated_cost_usd().unwrap() > 0.0);
    }

    #[test]
    fn test_openai_gpt4o_mini_cheaper_than_gpt4o() {
        let mini = LlmResponse {
            content: "x".into(),
            input_tokens: 1000,
            output_tokens: 500,
            provider: Provider::OpenAI,
            model: "gpt-4o-mini".into(),
        };
        let full = LlmResponse {
            content: "x".into(),
            input_tokens: 1000,
            output_tokens: 500,
            provider: Provider::OpenAI,
            model: "gpt-4o".into(),
        };
        assert!(mini.estimated_cost_usd().unwrap() < full.estimated_cost_usd().unwrap());
    }

    #[test]
    fn test_anthropic_haiku_cost_positive() {
        let r = LlmResponse {
            content: "x".into(),
            input_tokens: 1000,
            output_tokens: 500,
            provider: Provider::Anthropic,
            model: "claude-3-haiku".into(),
        };
        assert!(r.estimated_cost_usd().unwrap() > 0.0);
    }

    #[test]
    fn test_unknown_model_returns_none() {
        let r = LlmResponse {
            content: "x".into(),
            input_tokens: 1000,
            output_tokens: 500,
            provider: Provider::OpenAI,
            model: "gpt-3.5-turbo".into(),
        };
        assert_eq!(r.estimated_cost_usd(), None);
    }
}
