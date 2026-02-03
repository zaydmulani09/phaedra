use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub struct AnthropicClient {
    pub api_key: String,
    pub model: String,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    system: &'a str,
    messages: Vec<Message<'a>>,
    temperature: f32,
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    content: Vec<ContentBlock>,
    usage: Usage,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: String,
}

#[derive(Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

impl AnthropicClient {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("failed to build reqwest client");
        Self {
            api_key: api_key.into(),
            model: model.into(),
            client,
        }
    }

    pub async fn chat(
        &self,
        system: &str,
        user: &str,
        temperature: f32,
        max_tokens: u32,
    ) -> Result<crate::provider::LlmResponse> {
        let body = ChatRequest {
            model: &self.model,
            max_tokens,
            system,
            messages: vec![Message { role: "user", content: user }],
            temperature,
        };

        let resp = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Anthropic request failed")?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error {status}: {text}");
        }

        let chat_resp: ChatResponse =
            resp.json().await.context("Anthropic response parse failed")?;
        let content = chat_resp
            .content
            .into_iter()
            .next()
            .map(|b| b.text)
            .unwrap_or_default();

        Ok(crate::provider::LlmResponse {
            content,
            input_tokens: chat_resp.usage.input_tokens,
            output_tokens: chat_resp.usage.output_tokens,
            provider: crate::provider::Provider::Anthropic,
            model: self.model.clone(),
        })
    }
}
