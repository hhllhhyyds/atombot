//! OpenAI API client wrapper.
//!
//! Wraps [`async_openai::Client`] with environment-based configuration
//! and request/response logging.

use std::env;

use async_openai::types::chat::{ChatCompletionRequestMessage, ChatCompletionTools};
use async_openai::{
    config::OpenAIConfig,
    types::chat::{CreateChatCompletionRequestArgs, CreateChatCompletionResponse},
    Client,
};

use crate::agent::AgentError;
use crate::log;

/// Client for making chat completions API calls.
///
/// Configured via environment variables:
/// - `OPENAI_API_KEY` (required)
/// - `OPENAI_API_BASE` (default: `https://api.minimax.chat/v1`)
/// - `OPENAI_MODEL` (default: `MiniMax-M2.7`)
pub struct ApiClient {
    client: Client<OpenAIConfig>,
    model: String,
}

impl ApiClient {
    /// Create a new API client from environment variables.
    ///
    /// # Panics
    /// Panics if `OPENAI_API_KEY` is not set.
    pub fn new() -> Self {
        let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");
        let api_base = env::var("OPENAI_API_BASE")
            .unwrap_or_else(|_| "https://api.minimax.chat/v1".to_string());
        let model = env::var("OPENAI_MODEL").unwrap_or_else(|_| "MiniMax-M2.7".to_string());

        eprintln!("[ApiClient] api_key prefix: {}", &api_key[..8.min(api_key.len())]);
        eprintln!("[ApiClient] api_base: {}", api_base);
        eprintln!("[ApiClient] model: {}", model);

        let config = OpenAIConfig::new()
            .with_api_key(&api_key)
            .with_api_base(&api_base);

        let http_client = reqwest::ClientBuilder::new()
            .user_agent("async-openai")
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .unwrap();

        Self {
            client: Client::with_config(config).with_http_client(http_client),
            model,
        }
    }

    /// Send a chat completion request with tools.
    ///
    /// Logs both the request and response via the [`log!`] macro.
    pub async fn chat(
        &self,
        messages: &[ChatCompletionRequestMessage],
        tools: &[ChatCompletionTools],
    ) -> Result<CreateChatCompletionResponse, AgentError> {
        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages(messages.to_vec())
            .tools(tools.to_vec())
            .build()
            .unwrap();

        let request_json = serde_json::to_string_pretty(&request).unwrap_or_default();
        log!("REQUEST", &request_json);
        eprintln!("[API] Sending request to {}...", self.model);

        let result = self.client
            .chat()
            .create(request)
            .await;

        match &result {
            Ok(resp) => {
                eprintln!("[API] Response received: {} choices", resp.choices.len());
            }
            Err(e) => {
                eprintln!("[API] Error: {}", e);
            }
        }

        result
            .map_err(|e| AgentError::Api(e.to_string()))
            .inspect(|resp| {
                let response_json = serde_json::to_string_pretty(resp).unwrap_or_default();
                log!("RESPONSE", &response_json);
            })
    }
}
