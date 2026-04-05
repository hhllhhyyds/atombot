use std::io::{self, Write};

use atombot::agent::api_client::ApiClient;
use atombot::agent::config::AgentConfig;
use atombot::agent::tools::{AllowedDirectoriesConfig, ReadFileTool, ToolRegistry};
use atombot::agent::Agent;

#[tokio::main]
async fn main() {
    let mut tool_registry = ToolRegistry::new();
    tool_registry.register(ReadFileTool::new(
        AllowedDirectoriesConfig::default().with_workspace("/Users/hhl/Documents/projects/atombot"),
    ));

    let mut agent = Agent::new(ApiClient::new(), tool_registry, AgentConfig::default())
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

        if let Err(e) = agent.chat(input).await {
            eprintln!("Error: {}", e);
        }
    }
}
