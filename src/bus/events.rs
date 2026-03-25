/// 从聊天渠道收到的消息（入站）
///
/// 对应 Python 版的 InboundMessage dataclass
#[derive(Debug, Clone)]
pub struct InboundMessage {
    /// 渠道名称，比如 "telegram"、"cli"
    pub channel: String,
    /// 发送者 ID
    pub sender_id: String,
    /// 会话/聊天 ID
    pub chat_id: String,
    /// 消息正文
    pub content: String,
}

impl InboundMessage {
    /// 生成唯一的 session key，用于区分不同会话
    ///
    /// 比如同一个 Telegram 群 和 CLI 是不同的 session
    pub fn session_key(&self) -> String {
        format!("{}:{}", self.channel, self.chat_id)
    }
}

/// 要发送给用户的消息（出站）
///
/// 对应 Python 版的 OutboundMessage dataclass
#[derive(Debug, Clone)]
pub struct OutboundMessage {
    pub channel: String,
    pub chat_id: String,
    pub content: String,
}
