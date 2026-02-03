pub mod anthropic;
pub mod cost_tracker;
pub mod ollama;
pub mod openai;
pub mod prompt;
pub mod provider;
pub mod seeds;

pub use cost_tracker::CostTracker;
pub use ollama::{OllamaClient, DEFAULT_MODEL, DEFAULT_OLLAMA_URL};
pub use provider::{LlmResponse, Provider};
pub use seeds::{parse_llm_response, GeneratedSeed};

pub enum LlmClient {
    Ollama(OllamaClient),
    OpenAI(openai::OpenAIClient),
    Anthropic(anthropic::AnthropicClient),
}

impl LlmClient {
    pub fn from_config(
        provider: &str,
        api_key: Option<&str>,
        model: &str,
        base_url: &str,
    ) -> Self {
        match Provider::from_str(provider) {
            Provider::OpenAI => {
                Self::OpenAI(openai::OpenAIClient::new(api_key.unwrap_or(""), model))
            }
            Provider::Anthropic => {
                Self::Anthropic(anthropic::AnthropicClient::new(api_key.unwrap_or(""), model))
            }
            Provider::Ollama => Self::Ollama(OllamaClient::new(base_url, model)),
        }
    }

    pub async fn chat(
        &self,
        system: &str,
        user: &str,
        temperature: f32,
        max_tokens: u32,
    ) -> anyhow::Result<LlmResponse> {
        match self {
            Self::Ollama(c) => {
                let text = c.chat(system, user, temperature, max_tokens as i32).await?;
                Ok(LlmResponse {
                    content: text,
                    input_tokens: 0,
                    output_tokens: 0,
                    provider: Provider::Ollama,
                    model: c.model.clone(),
                })
            }
            Self::OpenAI(c) => c.chat(system, user, temperature, max_tokens).await,
            Self::Anthropic(c) => c.chat(system, user, temperature, max_tokens).await,
        }
    }

    pub async fn is_available(&self) -> bool {
        match self {
            Self::Ollama(c) => c.is_available().await,
            Self::OpenAI(c) => !c.api_key.is_empty(),
            Self::Anthropic(c) => !c.api_key.is_empty(),
        }
    }
}

/// Generate seeds using any LlmClient backend.
/// Returns the seeds and the raw LlmResponse (for cost tracking by the caller).
pub async fn generate_seeds_with_client(
    description: &str,
    count: usize,
    client: &LlmClient,
) -> (Vec<GeneratedSeed>, Option<LlmResponse>) {
    if !client.is_available().await {
        tracing::warn!("LLM client not available — skipping seed generation");
        return (vec![], None);
    }
    let system = prompt::seed_generation_system();
    let user = prompt::seed_generation_user(description, count);
    match client.chat(system, &user, 0.7, 2048).await {
        Ok(response) => {
            let seeds = seeds::parse_llm_response(&response.content);
            tracing::info!("LLM generated {} seeds", seeds.len());
            (seeds, Some(response))
        }
        Err(e) => {
            tracing::warn!("LLM seed generation failed: {e}");
            (vec![], None)
        }
    }
}

/// Generate seed inputs using Ollama (legacy entry point).
pub async fn generate_seeds(
    description: &str,
    count: usize,
    client: &OllamaClient,
) -> Vec<GeneratedSeed> {
    if !client.is_available().await {
        tracing::warn!(
            "Ollama not available at {} — skipping LLM seed generation",
            client.base_url
        );
        return vec![];
    }
    tracing::info!("Generating {} seeds via Ollama ({})...", count, client.model);
    let system = prompt::seed_generation_system();
    let user = prompt::seed_generation_user(description, count);
    match client.chat(system, &user, 0.7, 2048).await {
        Ok(response) => {
            tracing::debug!("LLM raw response:\n{}", response);
            let seeds = seeds::parse_llm_response(&response);
            tracing::info!("LLM generated {} valid seeds", seeds.len());
            seeds
        }
        Err(e) => {
            tracing::warn!("LLM seed generation failed: {e} — falling back to default seed");
            vec![]
        }
    }
}
