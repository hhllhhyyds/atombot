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

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("API error: {0}")]
    Api(String),
    #[error("Tool error: {0}")]
    Tool(String),
    #[error("Max tool iterations exceeded")]
    MaxIterations,
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

    pub async fn chat(&mut self, input: &str) -> Result<String, AgentError> {
        self.messages.push(
            ChatCompletionRequestUserMessageArgs::default()
                .content(input)
                .build()
                .unwrap()
                .into(),
        );

        for _ in 0..self.config.tool_max_iterations {
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

pub mod api_client;
pub mod config;
pub mod message_window;
pub mod tools;
