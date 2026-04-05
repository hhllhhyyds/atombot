use std::env;

use std::io::{self, Write};

use async_openai::types::chat::{ChatCompletionRequestMessage, ChatCompletionTools};
use async_openai::{
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionMessageToolCalls, ChatCompletionRequestAssistantMessageArgs,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestToolMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
        CreateChatCompletionResponse,
    },
    Client,
};

use atombot::agent::tools::{AllowedDirectoriesConfig, ReadFileTool, ToolRegistry};
use atombot::log;

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("API error: {0}")]
    Api(String),
    #[error("Tool error: {0}")]
    Tool(String),
    #[error("Max tool iterations exceeded")]
    MaxIterations,
}

#[derive(Debug, Clone, Copy)]
pub struct AgentConfig {
    pub tool_max_iterations: usize,
    pub max_messages: usize,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            tool_max_iterations: 10,
            max_messages: 40,
        }
    }
}

impl AgentConfig {
    pub fn with_tool_max_iterations(mut self, max: usize) -> Self {
        self.tool_max_iterations = max;
        self
    }

    pub fn with_max_messages(mut self, max: usize) -> Self {
        self.max_messages = max;
        self
    }
}

/// Message window that keeps recent messages and prunes old ones at legal boundaries.
///
/// A "legal" boundary is at a user turn - we never cut in the middle of a
/// tool call / tool result exchange to avoid orphaned tool results.
struct MessageWindow;

impl MessageWindow {
    /// Prune messages to fit within max limit, keeping system message and recent turns.
    fn prune(messages: &mut Vec<ChatCompletionRequestMessage>, max: usize) {
        if messages.len() <= max {
            return;
        }

        // Always keep system message (index 0)
        let system_len = if Self::is_system_message(&messages[0]) {
            1
        } else {
            0
        };

        // If we're already within limit even after removing everything before system, nothing to do
        if messages.len() <= max {
            return;
        }

        // Find the starting point: we want to keep the last (max - system_len) messages
        // but we must align to a user turn boundary
        let target_keep = max.saturating_sub(system_len);
        let prune_start = messages.len() - target_keep;

        // Find the nearest user turn at or before prune_start
        let mut actual_start = prune_start;
        for i in (system_len..prune_start).rev() {
            if Self::is_user_message(&messages[i]) {
                actual_start = i;
                break;
            }
        }

        // Remove messages from [system_len..actual_start)
        if actual_start > system_len {
            messages.drain(system_len..actual_start);
        }
    }

    fn is_system_message(msg: &ChatCompletionRequestMessage) -> bool {
        matches!(
            msg,
            ChatCompletionRequestMessage::System(_)
        )
    }

    fn is_user_message(msg: &ChatCompletionRequestMessage) -> bool {
        matches!(
            msg,
            ChatCompletionRequestMessage::User(_)
        )
    }
}

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

pub struct Agent {
    client: ApiClient,
    tool_registry: ToolRegistry,
    config: AgentConfig,
    messages: Vec<ChatCompletionRequestMessage>,
}

impl Agent {
    pub fn new(client: ApiClient, tool_registry: ToolRegistry, config: AgentConfig) -> Self {
        Self {
            client,
            tool_registry,
            config,
            messages: Vec::new(),
        }
    }

    pub fn with_system_prompt(mut self, prompt: &str) -> Self {
        self.messages.push(
            ChatCompletionRequestSystemMessageArgs::default()
                .content(prompt)
                .build()
                .unwrap()
                .into(),
        );
        log!("SYSTEM PROMPT", prompt);
        self
    }

    pub async fn run(&mut self, input: &str) -> Result<String, AgentError> {
        self.messages.push(
            ChatCompletionRequestUserMessageArgs::default()
                .content(input)
                .build()
                .unwrap()
                .into(),
        );

        for _ in 0..self.config.tool_max_iterations {
            // Prune old messages to prevent prompt size from growing indefinitely
            MessageWindow::prune(&mut self.messages, self.config.max_messages);

            let tools = self.tool_registry.build_chat_completion_tools();
            let response = self.client.chat(&self.messages, &tools).await?;

            let choice = response
                .choices
                .first()
                .ok_or_else(|| AgentError::Api("No choice in response".to_string()))?;
            let msg = &choice.message;

            // Handle tool calls
            if let Some(tool_calls) = &msg.tool_calls {
                let tool_calls: &[ChatCompletionMessageToolCalls] = tool_calls;
                println!("\n[工具调用: {}]", tool_calls.len());
                self.handle_tool_calls(tool_calls).await?;
                continue;
            }

            // Handle text response
            if let Some(content) = &msg.content {
                let content: String = content.clone();
                println!("\nAI: {}\n", content);

                self.messages.push(
                    ChatCompletionRequestAssistantMessageArgs::default()
                        .content(content.clone())
                        .build()
                        .unwrap()
                        .into(),
                );
                return Ok(content);
            }

            // Neither tool calls nor content - return error
            return Err(AgentError::Api("Empty response".to_string()));
        }

        Err(AgentError::MaxIterations)
    }

    async fn handle_tool_calls(
        &mut self,
        tool_calls: &[ChatCompletionMessageToolCalls],
    ) -> Result<(), AgentError> {
        for tool_call in tool_calls {
            match tool_call {
                ChatCompletionMessageToolCalls::Function(tc) => {
                    let name = &tc.function.name;
                    let args = &tc.function.arguments;
                    let args_json: serde_json::Value =
                        serde_json::from_str(args).unwrap_or_default();

                    let result = self.tool_registry.execute(name, args_json).await;

                    let (result_str, log_msg) = match &result {
                        Ok(r) => (r.clone(), format!("Tool: {}\nResult: {}", name, r)),
                        Err(e) => (
                            format!("工具执行失败: {}", e),
                            format!("Tool: {}\nError: {}", name, e),
                        ),
                    };

                    log!("TOOL EXEC", &log_msg);

                    self.messages.push(
                        ChatCompletionRequestAssistantMessageArgs::default()
                            .tool_calls(tool_calls.to_vec())
                            .build()
                            .unwrap()
                            .into(),
                    );
                    self.messages.push(
                        ChatCompletionRequestToolMessageArgs::default()
                            .content(result_str)
                            .tool_call_id(tc.id.clone())
                            .build()
                            .unwrap()
                            .into(),
                    );
                }
                ChatCompletionMessageToolCalls::Custom(ctc) => {
                    return Err(AgentError::Tool(format!(
                        "Custom tool calls not supported: {}",
                        ctc.custom_tool.name
                    )))
                }
            }
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let api_client = ApiClient::new();

    let mut tool_registry = ToolRegistry::new();
    tool_registry.register(ReadFileTool::new(
        AllowedDirectoriesConfig::default().with_workspace("/Users/hhl/Documents/projects/atombot"),
    ));

    let mut agent = Agent::new(api_client, tool_registry, AgentConfig::default())
        .with_system_prompt("你是一个有用的助手。当用户要求读取文件时，请使用 read_file 工具。");

    println!("开始对话，输入你的问题 (输入 quit 退出):\n");

    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        if input == "quit" || input == "exit" {
            println!("再见!");
            break;
        }

        if let Err(e) = agent.run(input).await {
            eprintln!("Error: {}", e);
        }
    }
}
