use atombot::agent::api_client::ApiClient;
use atombot::agent::config::AgentConfig;
use atombot::agent::tools::ToolRegistry;
use atombot::agent::Agent;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tauri::State;
use tokio::sync::Mutex;
use tokio::time::timeout;

fn find_env_file() -> Option<PathBuf> {
    let binding = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = binding.parent()?.parent()?;
    let env_path = workspace_root.join(".env");
    if env_path.exists() {
        Some(env_path)
    } else {
        None
    }
}

pub struct AppState {
    agent: Mutex<Agent>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatRequest {
    prompt: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatResponse {
    response: String,
    error: Option<String>,
}

#[tauri::command]
async fn chat(state: State<'_, AppState>, request: ChatRequest) -> Result<ChatResponse, String> {
    eprintln!("[DEBUG] Received chat request: {}", request.prompt);
    let mut agent = state.agent.lock().await;
    eprintln!("[DEBUG] Agent locked, calling chat with 60s timeout...");

    let result = timeout(Duration::from_secs(180), agent.chat(&request.prompt)).await;

    match result {
        Ok(Ok(response)) => {
            eprintln!(
                "[DEBUG] Chat succeeded, response length: {}",
                response.len()
            );
            Ok(ChatResponse {
                response,
                error: None,
            })
        }
        Ok(Err(e)) => {
            eprintln!("[DEBUG] Chat error: {}", e);
            Ok(ChatResponse {
                response: String::new(),
                error: Some(e.to_string()),
            })
        }
        Err(_) => {
            eprintln!("[DEBUG] Chat timed out after 60s");
            Ok(ChatResponse {
                response: String::new(),
                error: Some("请求超时 (60秒)".to_string()),
            })
        }
    }
}

pub fn create_app_state() -> AppState {
    let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "not-set".to_string());
    let api_base = std::env::var("OPENAI_API_BASE").unwrap_or_else(|_| "not-set".to_string());
    let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "not-set".to_string());
    eprintln!(
        "[DEBUG] ApiClient config: key={}, base={}, model={}",
        &api_key[..8.min(api_key.len())],
        api_base,
        model
    );

    let api_client = ApiClient::new();

    // Workspace is 3 levels up from src-tauri: src-tauri -> tauri_ui -> atombot
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let tool_registry = ToolRegistry::with_defaults(&workspace);

    let agent = Agent::new(api_client, tool_registry, AgentConfig::default())
        .with_system_prompt("你是一个有用的助手。");

    AppState {
        agent: Mutex::new(agent),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load .env BEFORE creating app state (so env vars are available during init)
    if let Some(env_path) = find_env_file() {
        eprintln!("[DEBUG] Loading .env from: {:?}", env_path);
        dotenvy::from_path(&env_path).ok();
    } else {
        eprintln!("[DEBUG] No .env file found, using environment variables");
    }

    let app_state = create_app_state();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![chat])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
