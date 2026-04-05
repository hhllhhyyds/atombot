use std::env;

use async_openai::types::chat::{ChatCompletionRequestMessage, ChatCompletionTools};
use async_openai::{
    config::OpenAIConfig,
    types::chat::{CreateChatCompletionRequestArgs, CreateChatCompletionResponse},
    Client,
};

use crate::agent::AgentError;
use crate::log;

pub struct ApiClient {
    client: Client<OpenAIConfig>,
    model: String,
}

impl ApiClient {
    pub fn new() -> Self {
        let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");
        let api_base = env::var("OPENAI_API_BASE")
            .unwrap_or_else(|_| "https://api.minimax.chat/v1".to_string());
        let model = env::var("OPENAI_MODEL").unwrap_or_else(|_| "MiniMax-M2.7".to_string());

        let config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base(api_base);

        let http_client = reqwest::ClientBuilder::new()
            .user_agent("async-openai")
            .build()
            .unwrap();

        Self {
            client: Client::with_config(config).with_http_client(http_client),
            model,
        }
    }

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

        self.client
            .chat()
            .create(request)
            .await
            .map_err(|e| AgentError::Api(e.to_string()))
            .inspect(|resp| {
                let response_json = serde_json::to_string_pretty(resp).unwrap_or_default();
                log!("RESPONSE", &response_json);
            })
    }
}
