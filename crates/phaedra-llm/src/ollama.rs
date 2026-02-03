use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
pub const DEFAULT_MODEL: &str = "llama3.2";

#[derive(Debug, Clone)]
pub struct OllamaClient {
    pub base_url: String,
    pub model: String,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    stream: bool,
    options: ChatOptions,
}

#[derive(Serialize)]
struct ChatOptions {
    temperature: f32,
    num_predict: i32,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize, Debug)]
struct ChatResponse {
    message: ChatResponseMessage,
}

#[derive(Deserialize, Debug)]
struct ChatResponseMessage {
    content: String,
}

impl OllamaClient {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("failed to build reqwest client");
        Self {
            base_url: base_url.into(),
            model: model.into(),
            client,
        }
    }

    pub fn default() -> Self {
        Self::new(DEFAULT_OLLAMA_URL, DEFAULT_MODEL)
    }

    pub async fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url);
        match self
            .client
            .get(&url)
            .timeout(Duration::from_secs(3))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    pub async fn chat(
        &self,
        system: &str,
        user: &str,
        temperature: f32,
        max_tokens: i32,
    ) -> Result<String> {
        let url = format!("{}/api/chat", self.base_url);
        let body = ChatRequest {
            model: &self.model,
            messages: vec![
                ChatMessage { role: "system", content: system },
                ChatMessage { role: "user", content: user },
            ],
            stream: false,
            options: ChatOptions {
                temperature,
                num_predict: max_tokens,
            },
        };

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("Ollama chat request failed")?;

        let chat_resp: ChatResponse = resp
            .json()
            .await
            .context("Ollama chat request failed")?;

        Ok(chat_resp.message.content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_construction() {
        let client = OllamaClient::new("http://localhost:11434", "llama3.2");
        assert_eq!(client.base_url, "http://localhost:11434");
        assert_eq!(client.model, "llama3.2");
    }

    #[test]
    fn test_default_client() {
        let client = OllamaClient::default();
        assert_eq!(client.base_url, DEFAULT_OLLAMA_URL);
        assert_eq!(client.model, DEFAULT_MODEL);
    }
}
