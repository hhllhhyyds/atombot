use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::State,
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};

use atombot::agent::api_client::ApiClient;
use atombot::agent::config::AgentConfig;
use atombot::agent::{
    tools::ToolRegistry,
    Agent,
};

#[derive(Clone)]
struct AppState {
    agent: Arc<Mutex<Agent>>,
}

#[derive(Debug, Deserialize)]
struct ChatRequest {
    prompt: String,
}

#[derive(Debug, Serialize)]
struct ChatResponse {
    response: String,
    error: Option<String>,
}

const HTML_PAGE: &str = r#"<!DOCTYPE html>
<html lang="zh">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Atombot</title>
    <script src="https://cdn.jsdelivr.net/npm/marked/marked.min.js"></script>
    <style>
        * {
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            max-width: 900px;
            margin: 0 auto;
            padding: 20px;
            background: #f5f5f5;
        }
        h1 {
            text-align: center;
            color: #333;
            margin-bottom: 20px;
        }
        #chat-container {
            background: white;
            border-radius: 8px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
            padding: 20px;
            min-height: 500px;
            display: flex;
            flex-direction: column;
        }
        #messages {
            flex: 1;
            overflow-y: auto;
            margin-bottom: 20px;
        }
        .message {
            padding: 12px 16px;
            border-radius: 12px;
            margin-bottom: 12px;
            max-width: 85%;
        }
        .user {
            background: #007AFF;
            color: white;
            align-self: flex-end;
            margin-left: auto;
        }
        .assistant {
            background: #E9E9EB;
            color: #333;
            align-self: flex-start;
        }
        .error {
            background: #FF3B30;
            color: white;
        }
        /* Markdown styles */
        .assistant h1, .assistant h2, .assistant h3, .assistant h4 {
            margin-top: 0.5em;
            margin-bottom: 0.5em;
            color: #222;
        }
        .assistant h1 { font-size: 1.4em; }
        .assistant h2 { font-size: 1.2em; }
        .assistant h3 { font-size: 1.1em; }
        .assistant p {
            margin: 0.5em 0;
            line-height: 1.5;
        }
        .assistant code {
            background: rgba(0,0,0,0.08);
            padding: 2px 6px;
            border-radius: 4px;
            font-family: 'SF Mono', Consolas, monospace;
            font-size: 0.9em;
        }
        .assistant pre {
            background: #1e1e1e;
            color: #d4d4d4;
            padding: 12px;
            border-radius: 8px;
            overflow-x: auto;
            margin: 0.8em 0;
        }
        .assistant pre code {
            background: transparent;
            padding: 0;
            color: inherit;
        }
        .assistant ul, .assistant ol {
            margin: 0.5em 0;
            padding-left: 1.5em;
        }
        .assistant li {
            margin: 0.25em 0;
        }
        .assistant blockquote {
            border-left: 3px solid #ccc;
            padding-left: 12px;
            margin: 0.5em 0;
            color: #666;
        }
        .assistant table {
            border-collapse: collapse;
            margin: 0.8em 0;
            width: 100%;
        }
        .assistant th, .assistant td {
            border: 1px solid #ddd;
            padding: 8px 12px;
            text-align: left;
        }
        .assistant th {
            background: #f0f0f0;
        }
        .assistant a {
            color: #007AFF;
        }
        .assistant hr {
            border: none;
            border-top: 1px solid #ddd;
            margin: 1em 0;
        }
        #input-area {
            display: flex;
            gap: 10px;
        }
        #prompt {
            flex: 1;
            padding: 12px;
            border: 1px solid #ddd;
            border-radius: 8px;
            font-size: 16px;
            resize: none;
            font-family: inherit;
        }
        #prompt:focus {
            outline: none;
            border-color: #007AFF;
        }
        #submit {
            padding: 12px 24px;
            background: #007AFF;
            color: white;
            border: none;
            border-radius: 8px;
            font-size: 16px;
            cursor: pointer;
        }
        #submit:hover {
            background: #0056CC;
        }
        #submit:disabled {
            background: #ccc;
            cursor: not-allowed;
        }
    </style>
</head>
<body>
    <h1>🤖 Atombot</h1>
    <div id="chat-container">
        <div id="messages"></div>
        <div id="input-area">
            <textarea id="prompt" rows="2" placeholder="输入你的问题..."></textarea>
            <button id="submit">发送</button>
        </div>
    </div>

    <script>
        const messagesDiv = document.getElementById('messages');
        const promptInput = document.getElementById('prompt');
        const submitBtn = document.getElementById('submit');

        let isLoading = false;

        function addMessage(role, content, isError = false) {
            const div = document.createElement('div');
            div.className = `message ${role}${isError ? ' error' : ''}`;

            if (role === 'assistant' && !isError) {
                div.innerHTML = marked.parse(content);
            } else {
                div.textContent = content;
            }

            messagesDiv.appendChild(div);
            messagesDiv.scrollTop = messagesDiv.scrollHeight;
        }

        async function sendMessage() {
            const prompt = promptInput.value.trim();
            if (!prompt || isLoading) return;

            isLoading = true;
            submitBtn.disabled = true;
            promptInput.value = '';

            addMessage('user', prompt);

            try {
                const response = await fetch('/chat', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ prompt })
                });

                const data = await response.json();

                if (data.error) {
                    addMessage('assistant', data.error, true);
                } else {
                    addMessage('assistant', data.response);
                }
            } catch (err) {
                addMessage('assistant', '请求失败: ' + err.message, true);
            } finally {
                isLoading = false;
                submitBtn.disabled = false;
                promptInput.focus();
            }
        }

        submitBtn.addEventListener('click', sendMessage);
        promptInput.addEventListener('keydown', (e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                sendMessage();
            }
        });
    </script>
</body>
</html>
"#;

async fn get_index() -> impl IntoResponse {
    Html(HTML_PAGE)
}

#[axum::debug_handler]
async fn chat_handler(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Json<ChatResponse> {
    let mut agent = state.agent.lock().await;

    match agent.chat(&req.prompt).await {
        Ok(response) => Json(ChatResponse {
            response,
            error: None,
        }),
        Err(e) => Json(ChatResponse {
            response: String::new(),
            error: Some(e.to_string()),
        }),
    }
}

fn create_app_state() -> AppState {
    let api_client = ApiClient::new();
    let tool_registry = ToolRegistry::with_defaults(env!("CARGO_MANIFEST_DIR"));

    let agent = Agent::new(api_client, tool_registry, AgentConfig::default())
        .with_system_prompt("你是一个有用的助手。当用户要求读取文件时，请使用 read_file 工具。");

    AppState {
        agent: Arc::new(Mutex::new(agent)),
    }
}

#[tokio::main]
async fn main() {
    // Load .env from workspace root
    let env_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.env");
    if env_path.exists() {
        dotenvy::from_path(&env_path).ok();
        println!("[DEBUG] Loaded .env from: {:?}", env_path);
    }

    let app = create_app_state();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let router = Router::new()
        .route("/", get(get_index))
        .route("/chat", post(chat_handler))
        .layer(cors)
        .with_state(app);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .unwrap();

    println!("🌐 Web UI 已启动: http://127.0.0.1:8080");
    println!("按 Ctrl+C 停止");

    axum::serve(listener, router).await.unwrap();
}
