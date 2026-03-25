use serde::{Deserialize, Serialize};

/// 消息角色
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// 系统消息
    System,
    /// 用户消息
    User,
    /// 助手消息
    Assistant,
    /// 工具返回结果
    Tool,
}

/// 与 LLM 交互的消息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// 角色
    pub role: Role,
    /// 内容
    pub content: String,
    /// 工具调用 ID（仅当 role 为 Tool 时需要）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// 工具名称（仅当 role 为 Tool 时需要）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Message {
    /// 创建一个用户消息
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            tool_call_id: None,
            name: None,
        }
    }

    /// 创建一个系统消息
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
            tool_call_id: None,
            name: None,
        }
    }

    /// 创建一个助手消息
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            tool_call_id: None,
            name: None,
        }
    }

    /// 创建一个工具返回消息
    pub fn tool(content: impl Into<String>, tool_call_id: impl Into<String>, name: Option<String>) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
            tool_call_id: Some(tool_call_id.into()),
            name,
        }
    }
}