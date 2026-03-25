use crate::provider::message::Message;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use async_trait::async_trait;

/// A tool call request from the LLM
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ToolCallRequest {
    id: String,
    name: String,
    args: HashMap<String, serde_json::Value>,
}

impl ToolCallRequest {
    pub fn new(id: String, name: String, args: HashMap<String, serde_json::Value>) -> Self {
        Self { id, name, args }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn args(&self) -> &HashMap<String, serde_json::Value> {
        &self.args
    }
}

/// Response from an LLM provider
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LLMResponse {
    content: Option<String>,
    tool_call: Vec<ToolCallRequest>,
    finish_reason: String,
    usage: HashMap<String, u32>,
}

impl LLMResponse {
    pub fn new(
        content: Option<String>,
        tool_call: Vec<ToolCallRequest>,
        finish_reason: String,
        usage: HashMap<String, u32>,
    ) -> Self {
        Self {
            content,
            tool_call,
            finish_reason,
            usage,
        }
    }

    pub fn content(&self) -> Option<&str> {
        self.content.as_deref()
    }

    pub fn tool_call(&self) -> &Vec<ToolCallRequest> {
        &self.tool_call
    }

    pub fn finish_reason(&self) -> &str {
        &self.finish_reason
    }

    pub fn usage(&self) -> &HashMap<String, u32> {
        &self.usage
    }

    pub fn has_tool_call(&self) -> bool {
        !self.tool_call.is_empty()
    }
}

/// Implementations should handle the specifics of each provider's API
/// while maintaining a consistent interface.
#[async_trait]
pub trait LLMProvider {
    async fn chat(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<HashMap<String, serde_json::Value>>>,
        model: Option<String>,
        max_tokens: u32,
        temperature: f64,
    ) -> Result<LLMResponse, anyhow::Error>;


    fn get_default_model(&self) -> String;
}
