use std::collections::HashMap;

use anyhow::{anyhow, Context};
use async_trait::async_trait;
use log::debug;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

use super::llm_provider::{LLMProvider, LLMResponse, ToolCallRequest};
use super::message::Message;

// ── OpenAI API 响应结构（仅用于反序列化） ────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ApiResponse {
    #[serde(default, alias = "choices")]
    choice: Vec<ApiChoice>,
    #[serde(default)]
    usage: Option<ApiUsage>,
}

#[derive(Debug, Deserialize)]
struct ApiChoice {
    message: ApiMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiMessage {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ApiToolCall>,
}

#[derive(Debug, Deserialize)]
struct ApiToolCall {
    id: String,
    function: ApiFunction,
}

#[derive(Debug, Deserialize)]
struct ApiFunction {
    name: String,
    /// LLM 返回的 arguments 是 JSON string需要二次解析
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct ApiUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
    #[serde(default)]
    total_tokens: u32,
}

// ── OpenAIProvider ────────────────────────────────────────────────────────────

pub struct OpenAIProvider {
    api_key: String,
    api_base: String,
    default_model: String,
    client: Client,
}

impl OpenAIProvider {
    /// 创建一个新的 OpenAIProvider
    ///
    /// - `api_key`：API 密钥
    /// - `api_base`：Base URL 不含 `/v1`（如 `https://api.openai.com`）
    /// - `default_model`：默认模型 ID（如 `gpt-4o`）
    pub fn new(api_key: String, api_base: Option<String>, default_model: Option<String>) -> Self {
        Self {
            api_key,
            api_base: api_base.unwrap_or_else(|| "https://api.openai.com".to_string()),
            default_model: default_model.unwrap_or_else(|| "gpt-4o".to_string()),
            client: Client::new(),
        }
    }

    fn parse_response(&self, resp: ApiResponse) -> Result<LLMResponse, anyhow::Error> {
        let choice = resp
            .choice
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("empty choice in API response"))?;
        // 解析 tool_call
        let mut tool_call = Vec::new();
        for tc in choice.message.tool_calls {
            // arguments 是 JSON string 反序列化成 map
            let args: HashMap<String, Value> = serde_json::from_str(&tc.function.arguments)
                .with_context(|| {
                    format!(
                        "failed to parse tool argument for '{}': {}",
                        tc.function.name, tc.function.arguments
                    )
                })?;

            tool_call.push(ToolCallRequest::new(tc.id, tc.function.name, args));
        }
        // 解析 usage
        let usage = if let Some(u) = resp.usage {
            let mut map = HashMap::new();
            map.insert("prompt_tokens".to_string(), u.prompt_tokens);
            map.insert("completion_tokens".to_string(), u.completion_tokens);
            map.insert("total_tokens".to_string(), u.total_tokens);
            map
        } else {
            HashMap::new()
        };
        Ok(LLMResponse::new(
            choice.message.content,
            tool_call,
            choice.finish_reason.unwrap_or_else(|| "stop".to_string()),
            usage,
        ))
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    async fn chat(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<HashMap<String, Value>>>,
        model: Option<String>,
        max_tokens: u32,
        temperature: f64,
    ) -> Result<LLMResponse, anyhow::Error> {
        let model = model.unwrap_or_else(|| self.default_model.clone());
        let url = format!(
            "{}/v1/chat/completions",
            self.api_base.trim_end_matches('/')
        );
        debug!("Request URL: {}, model: {}", url, model);
        // 将 Message 转换为 HashMap 供 serde_json 序列化
        let messages: Vec<HashMap<String, Value>> = messages
            .into_iter()
            .map(|m| {
                let mut map = HashMap::new();
                map.insert(
                    "role".to_string(),
                    json!(format!("{:?}", m.role).to_lowercase()),
                );
                map.insert("content".to_string(), json!(m.content));
                if let Some(tool_call_id) = m.tool_call_id {
                    map.insert("tool_call_id".to_string(), json!(tool_call_id));
                }
                if let Some(name) = m.name {
                    map.insert("name".to_string(), json!(name));
                }
                map
            })
            .collect();
        let mut body = json!({
            "model": model,
            "messages": messages,
            "max_tokens": max_tokens,
            "temperature": temperature,
        });
        if let Some(tools) = tools {
            body["tools"] = json!(tools);
            body["tool_choice"] = json!("auto");
        }
        debug!("Sending request to {}\nbody:\n{}", url, body);
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .context("HTTP request failed")?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("API error {}: {}", status, text));
        }
        let api_resp: ApiResponse = resp.json().await.context("failed to parse API response")?;
        self.parse_response(api_resp)
    }

    fn get_default_model(&self) -> String {
        self.default_model.clone()
    }
}
