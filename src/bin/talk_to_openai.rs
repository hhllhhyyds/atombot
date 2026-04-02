use std::env;

use async_openai::{
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
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

    let messages: Vec<async_openai::types::chat::ChatCompletionRequestMessage> = vec![
        ChatCompletionRequestSystemMessageArgs::default()
            .content("你是个诚实的 AI")
            .build()
            .unwrap()
            .into(),
        ChatCompletionRequestUserMessageArgs::default()
            .content("你是哪个大模型？告诉我 sin 23 度是多少")
            .build()
            .unwrap()
            .into(),
    ];

    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages(messages)
        .build()
        .unwrap();

    let response = client.chat().create(request).await.unwrap();

    println!("{:?}", response);
}
