use std::env;
use std::io::{self, Write};

use async_openai::{
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    },
    Client,
};

#[tokio::main]
async fn main() {
    let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");
    let api_base =
        env::var("OPENAI_API_BASE").unwrap_or_else(|_| "https://api.minimax.chat/v1".to_string());
    let model = env::var("OPENAI_MODEL").unwrap_or_else(|_| "MiniMax-M2.7".to_string());

    let config = OpenAIConfig::new()
        .with_api_key(api_key)
        .with_api_base(api_base);

    // Use custom reqwest client
    let http_client = reqwest::ClientBuilder::new()
        .user_agent("async-openai")
        .build()
        .unwrap();
    let client = Client::with_config(config).with_http_client(http_client);

    let mut messages: Vec<async_openai::types::chat::ChatCompletionRequestMessage> =
        vec![ChatCompletionRequestSystemMessageArgs::default()
            .content("你是个诚实的 AI")
            .build()
            .unwrap()
            .into()];

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

        let request = CreateChatCompletionRequestArgs::default()
            .model(&model)
            .messages(messages.clone())
            .build()
            .unwrap();

        let response = client.chat().create(request).await.unwrap();

        if let Some(choice) = response.choices.first() {
            if let Some(content) = &choice.message.content {
                println!("\nAI: {}\n", content);

                // 将 AI 的回复添加到消息历史中
                messages.push(
                    ChatCompletionRequestAssistantMessageArgs::default()
                        .content(content.clone())
                        .build()
                        .unwrap()
                        .into(),
                );
            }
        }
    }
}
