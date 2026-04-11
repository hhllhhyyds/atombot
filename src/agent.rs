//! Core agent — orchestrates multi-turn conversations with tool calling.
//!
//! The [`Agent`] struct holds conversation state and drives the
//! tool-calling loop: receive user input → call LLM → execute tools → repeat.
//!
//! # Example
//! ```ignore
//! let agent = Agent::new(client, registry, config)
//!     .with_system_prompt("You are a helpful assistant.");
//! let response = agent.chat("Hello!").await?;
//! ```

use async_openai::types::chat::ChatCompletionRequestMessage;
use async_openai::types::chat::{
    ChatCompletionMessageToolCalls, ChatCompletionRequestAssistantMessageArgs,
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestToolMessageArgs,
    ChatCompletionRequestUserMessageArgs,
};

use crate::agent::api_client::ApiClient;
use crate::agent::config::AgentConfig;
use crate::agent::message_window::MessageWindow;
use crate::agent::tools::ToolRegistry;
use crate::log;

/// Errors that can occur during agent execution.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// OpenAI API call failed
    #[error("API error: {0}")]
    Api(String),
    /// Tool execution failed
    #[error("Tool error: {0}")]
    Tool(String),
    /// Tool-call loop exceeded max iterations (potential infinite loop guard)
    #[error("Max tool iterations exceeded")]
    MaxIterations,
}

/// The main agent struct — maintains conversation history and executes tools.
pub struct Agent {
    /// Client for making OpenAI API calls
    client: ApiClient,
    /// Registry of available tools
    tool_registry: ToolRegistry,
    /// Agent configuration (max iterations, message window size)
    config: AgentConfig,
    /// Full conversation history (system + user + assistant + tool messages)
    messages: Vec<ChatCompletionRequestMessage>,
}

impl Agent {
    /// Create a new agent with the given client, tool registry, and config.
    pub fn new(client: ApiClient, tool_registry: ToolRegistry, config: AgentConfig) -> Self {
        Self {
            client,
            tool_registry,
            config,
            messages: Vec::new(),
        }
    }

    /// Add a system prompt to the conversation.
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

    /// Send a user message and get the agent's response.
    ///
    /// This drives the tool-calling loop:
    /// 1. Append user message to history
    /// 2. Prune history if it exceeds [`AgentConfig::max_messages`]
    /// 3. Call LLM with current tools
    /// 4. If LLM requests tool calls → execute and loop
    /// 5. If LLM returns text → return it
    pub async fn chat(&mut self, input: &str) -> Result<String, AgentError> {
        self.messages.push(
            ChatCompletionRequestUserMessageArgs::default()
                .content(input)
                .build()
                .unwrap()
                .into(),
        );

        for iteration in 0..self.config.tool_max_iterations {
            // Prune old messages to keep context window manageable
            MessageWindow::prune(&mut self.messages, self.config.max_messages);

            // Log current message state for debugging
            eprintln!("\n========== [迭代 {}] 消息历史 ({}条) ==========", iteration + 1, self.messages.len());
            for (i, msg) in self.messages.iter().enumerate() {
                match msg {
                    ChatCompletionRequestMessage::User(m) => {
                        let snippet = match &m.content {
                            async_openai::types::chat::ChatCompletionRequestUserMessageContent::Text(t) =>
                                if t.chars().count() > 80 { format!("{}...", t.chars().take(80).collect::<String>()) } else { t.clone() },
                            async_openai::types::chat::ChatCompletionRequestUserMessageContent::Array(_) =>
                                "[array content]".to_string(),
                        };
                        eprintln!("  [{:2}] USER: {}", i, snippet);
                    }
                    ChatCompletionRequestMessage::Assistant(m) => {
                        if let Some(tc) = &m.tool_calls {
                            let names: Vec<_> = tc.iter().map(|t| match t {
                                ChatCompletionMessageToolCalls::Function(f) => f.function.name.clone(),
                                ChatCompletionMessageToolCalls::Custom(c) => c.custom_tool.name.clone(),
                            }).collect();
                            eprintln!("  [{:2}] ASSISTANT: tool_calls={}", i, names.join(", "));
                        } else {
                            let snippet = match &m.content {
                                Some(async_openai::types::chat::ChatCompletionRequestAssistantMessageContent::Text(t)) =>
                                    if t.chars().count() > 80 { format!("{}...", t.chars().take(80).collect::<String>()) } else { t.clone() },
                                _ => "(no content)".to_string(),
                            };
                            eprintln!("  [{:2}] ASSISTANT: {}", i, snippet);
                        }
                    }
                    ChatCompletionRequestMessage::Tool(m) => {
                        let snippet = match &m.content {
                            async_openai::types::chat::ChatCompletionRequestToolMessageContent::Text(t) =>
                                if t.chars().count() > 100 { format!("{}...", t.chars().take(100).collect::<String>()) } else { t.clone() },
                            async_openai::types::chat::ChatCompletionRequestToolMessageContent::Array(_) =>
                                "[array content]".to_string(),
                        };
                        eprintln!("  [{:2}] TOOL({}): {}", i, m.tool_call_id, snippet.replace('\n', " "));
                    }
                    ChatCompletionRequestMessage::System(m) => {
                        let snippet = match &m.content {
                            async_openai::types::chat::ChatCompletionRequestSystemMessageContent::Text(t) =>
                                if t.chars().count() > 80 { format!("{}...", t.chars().take(80).collect::<String>()) } else { t.clone() },
                            async_openai::types::chat::ChatCompletionRequestSystemMessageContent::Array(_) =>
                                "[array content]".to_string(),
                        };
                        eprintln!("  [{:2}] SYSTEM: {}", i, snippet);
                    }
                    _ => eprintln!("  [{:2}] OTHER", i),
                }
            }

            let tools = self.tool_registry.build_chat_completion_tools();
            let response = self.client.chat(&self.messages, &tools).await?;

            let choice = response
                .choices
                .first()
                .ok_or_else(|| AgentError::Api("No choice in response".to_string()))?;
            let msg = &choice.message;

            // Log model's raw response for debugging
            let resp_content = msg.content.as_ref().map(|c| c.as_str()).unwrap_or("(none)").chars().take(200).collect::<String>();
            let resp_tools: Vec<String> = msg.tool_calls.as_ref().map(|tc| tc.iter().map(|t| match t {
                ChatCompletionMessageToolCalls::Function(f) => format!("{}(...)", f.function.name),
                ChatCompletionMessageToolCalls::Custom(c) => c.custom_tool.name.clone(),
            }).collect()).unwrap_or_default();
            eprintln!("\n>>> 模型响应: content=\"{}\" tool_calls={:?}", resp_content, resp_tools);

            // LLM requested tool calls — execute them all
            if let Some(tool_calls) = &msg.tool_calls {
                let tool_calls: &[ChatCompletionMessageToolCalls] = tool_calls;
                println!(
                    "\n[工具调用: {}, {}]",
                    tool_calls.len(),
                    tool_calls
                        .iter()
                        .map(|tc| match tc {
                            ChatCompletionMessageToolCalls::Function(f) => f.function.name.clone(),
                            ChatCompletionMessageToolCalls::Custom(c) => c.custom_tool.name.clone(),
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                self.handle_tool_calls(tool_calls).await?;
                continue;
            }

            // LLM returned text — this is our final response
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

            // Neither tool calls nor content — unexpected response
            return Err(AgentError::Api("Empty response".to_string()));
        }

        Err(AgentError::MaxIterations)
    }

    /// Execute a batch of tool calls and append results to the message history.
    ///
    /// For each tool call:
    /// 1. Execute the tool with the provided arguments
    /// 2. Log the result
    /// 3. Append assistant message (with tool call) + tool result message to history
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

                    // Execute the tool
                    let result = self.tool_registry.execute(name, args_json).await;

                    let (result_str, log_msg) = match &result {
                        Ok(r) => (r.clone(), format!("Tool: {}\nResult: {}", name, r)),
                        Err(e) => (
                            format!("工具执行失败: {}", e),
                            format!("Tool: {}\nError: {}", name, e),
                        ),
                    };

                    log!("TOOL EXEC", &log_msg);

                    // Append assistant message with this single tool call
                    self.messages.push(
                        ChatCompletionRequestAssistantMessageArgs::default()
                            .tool_calls(vec![tool_call.clone()])
                            .build()
                            .unwrap()
                            .into(),
                    );
                    // Append the tool's result as a separate message
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

pub mod api_client;
pub mod config;
pub mod message_window;
pub mod tools;
