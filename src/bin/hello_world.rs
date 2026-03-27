use anyhow::Result;
use atombot::provider::{LLMProvider, Message, OpenAIProvider};
use std::io::{self, Write};

fn init_env_logger() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .format_timestamp_millis()
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    init_env_logger();
    println!("=== Atombot CLI Demo ===");
    println!("Press Ctrl+C to exit\n");

    // 从环境变量读取 API key
    let api_key =
        std::env::var("OPENAI_API_KEY").expect("Please set OPENAI_API_KEY environment variable");

    // 可选配置：API base 和模型
    let api_base = std::env::var("OPENAI_API_BASE").ok();
    let model = std::env::var("OPENAI_MODEL").ok();

    // 创建 OpenAI provider
    let provider = OpenAIProvider::new(api_key, api_base, model);

    // 初始化消息历史（包含系统提示）
    let mut messages = vec![Message::system("You are a helpful assistant.")];

    loop {
        // 读取用户输入
        print!("> ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        // 添加用户消息
        messages.push(Message::user(input));

        // 调用 LLM
        print!("🤖 ");
        io::stdout().flush()?;

        let response = provider
            .chat(
                messages.clone(),
                None, // 不使用 tools
                None, // 使用默认模型
                1024, // max_tokens
                0.7,  // temperature
            )
            .await?;

        // 打印回复
        if let Some(content) = response.content() {
            println!("{}", content);
            // 添加助手消息到历史
            messages.push(Message::assistant(content));
        }

        println!();
    }
}
