use std::env;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::Path;

use async_openai::{
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionMessageToolCalls, ChatCompletionRequestAssistantMessageArgs,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestToolMessageArgs,
        ChatCompletionRequestUserMessageArgs, ChatCompletionTool, ChatCompletionTools,
        CreateChatCompletionRequestArgs, FunctionObject,
    },
    Client,
};

const MAX_ITERATIONS: usize = 10;
const LOG_FILE: &str = "talk_to_openai.log";

fn init_log() -> BufWriter<File> {
    let log_path = Path::new(LOG_FILE);
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .expect("Failed to open log file");
    BufWriter::new(file)
}

fn log_write(log: &mut BufWriter<File>, prefix: &str, content: &str) {
    let timestamp = chrono_lite_timestamp();
    writeln!(log, "\n=== {} [{}] ===", prefix, timestamp).ok();
    writeln!(log, "{}", content).ok();
    writeln!(log, "=== END {} ===", prefix).ok();
    log.flush().ok();
}

fn chrono_lite_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    let millis = dur.subsec_millis();
    format!("{}.{:03}", secs, millis)
}

fn build_read_file_tool() -> ChatCompletionTools {
    ChatCompletionTools::Function(ChatCompletionTool {
        function: FunctionObject {
            name: "read_file".into(),
            description: Some("Read the contents of a file".into()),
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The file path to read"
                    }
                },
                "required": ["path"]
            })),
            strict: None,
        },
    })
}

async fn execute_read_file(path: &str) -> String {
    let path = Path::new(path);
    if !path.exists() {
        return format!("Error: File not found: {}", path.display());
    }
    if !path.is_file() {
        return format!("Error: Not a file: {}", path.display());
    }
    match std::fs::read_to_string(path) {
        Ok(content) => {
            if content.len() > 5000 {
                format!(
                    "{}...\n\n(truncated, showing first 5000 chars)",
                    &content[..5000]
                )
            } else {
                content
            }
        }
        Err(e) => format!("Error reading file: {}", e),
    }
}

#[tokio::main]
async fn main() {
    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");
    let api_base =
        env::var("OPENAI_API_BASE").unwrap_or_else(|_| "https://api.minimax.chat/v1".to_string());
    let model = env::var("OPENAI_MODEL").unwrap_or_else(|_| "MiniMax-M2.7".to_string());

    let config = OpenAIConfig::new()
        .with_api_key(api_key)
        .with_api_base(api_base);

    let http_client = reqwest::ClientBuilder::new()
        .user_agent("async-openai")
        .build()
        .unwrap();
    let client = Client::with_config(config).with_http_client(http_client);

    let tools = vec![build_read_file_tool()];

    let system_prompt = "你是一个有用的助手。当用户要求读取文件时，请使用 read_file 工具。";

    let mut messages: Vec<async_openai::types::chat::ChatCompletionRequestMessage> =
        vec![ChatCompletionRequestSystemMessageArgs::default()
            .content(system_prompt)
            .build()
            .unwrap()
            .into()];

    // Log system prompt
    let mut log_file = init_log();
    log_write(&mut log_file, "SYSTEM PROMPT", system_prompt);

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

        messages.push(
            ChatCompletionRequestUserMessageArgs::default()
                .content(input)
                .build()
                .unwrap()
                .into(),
        );

        // 对话循环，支持工具调用
        for _iteration in 0..MAX_ITERATIONS {
            let request = CreateChatCompletionRequestArgs::default()
                .model(&model)
                .messages(messages.clone())
                .tools(tools.clone())
                .build()
                .unwrap();

            // Log request
            let request_json = serde_json::to_string_pretty(&request).unwrap_or_default();
            log_write(&mut log_file, "REQUEST", &request_json);

            let response = client.chat().create(request).await.unwrap();

            // Log response
            let response_json = serde_json::to_string_pretty(&response).unwrap_or_default();
            log_write(&mut log_file, "RESPONSE", &response_json);

            if let Some(choice) = response.choices.first() {
                let msg = &choice.message;

                // 如果有工具调用
                if let Some(tool_calls) = &msg.tool_calls {
                    println!("\n[工具调用: {}]", tool_calls.len());

                    for tool_call in tool_calls {
                        match tool_call {
                            ChatCompletionMessageToolCalls::Function(tc) => {
                                let name = &tc.function.name;
                                let args = &tc.function.arguments;
                                let args_json: serde_json::Value =
                                    serde_json::from_str(args).unwrap_or_default();
                                let path =
                                    args_json.get("path").and_then(|v| v.as_str()).unwrap_or("");

                                println!("  - {}(path='{}')", name, path);

                                let result = execute_read_file(path).await;

                                // Log tool execution
                                let tool_exec_log =
                                    format!("Tool: {}\nPath: {}\nResult: {}", name, path, result);
                                log_write(&mut log_file, "TOOL EXEC", &tool_exec_log);

                                // 添加工具结果消息
                                messages.push(
                                    ChatCompletionRequestAssistantMessageArgs::default()
                                        .tool_calls(tool_calls.clone())
                                        .build()
                                        .unwrap()
                                        .into(),
                                );
                                messages.push(
                                    ChatCompletionRequestToolMessageArgs::default()
                                        .content(result)
                                        .tool_call_id(tc.id.clone())
                                        .build()
                                        .unwrap()
                                        .into(),
                                );
                            }
                            ChatCompletionMessageToolCalls::Custom(_) => {
                                println!("  - Custom tool call (not supported)")
                            }
                        }
                    }
                    continue; // 继续循环获取下一个响应
                }

                // 普通文本回复
                if let Some(content) = &msg.content {
                    println!("\nAI: {}\n", content);

                    messages.push(
                        ChatCompletionRequestAssistantMessageArgs::default()
                            .content(content.clone())
                            .build()
                            .unwrap()
                            .into(),
                    );
                    break; // 对话完成
                }
            }
        }
    }
}
