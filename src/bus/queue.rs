use tokio::sync::{broadcast, mpsc};

use crate::bus::events::{InboundMessage, OutboundMessage};

/// 消息总线
///
/// 解耦渠道（Telegram/CLI/...）和 Agent 核心。
///
/// 数据流：
///   渠道 --[inbound_tx]--> [队列] --[inbound_rx]--> AgentLoop
///   AgentLoop --[outbound_tx]--> [队列] --[outbound_rx]--> 各渠道订阅者
pub struct MessageBus {
    /// 入站：Agent 从这里收消息
    pub inbound_rx: mpsc::Receiver<InboundMessage>,
    /// 入站发送端：渠道用这个往队列里塞消息（可 clone 给多个渠道）
    pub inbound_tx: mpsc::Sender<InboundMessage>,

    /// 出站：Agent 用这个发回复
    pub outbound_tx: broadcast::Sender<OutboundMessage>,
}

impl MessageBus {
    pub fn new() -> Self {
        // 入站队列容量 32，超出会背压（caller 的 send().await 会等待）
        let (inbound_tx, inbound_rx) = mpsc::channel(32);

        // 出站广播容量 32，订阅慢了会丢消息（broadcast 的特性）
        let (outbound_tx, _) = broadcast::channel(32);

        Self {
            inbound_rx,
            inbound_tx,
            outbound_tx,
        }
    }

    /// 订阅出站消息（每个渠道调一次，拿到自己的 Receiver）
    pub fn subscribe_outbound(&self) -> broadcast::Receiver<OutboundMessage> {
        self.outbound_tx.subscribe()
    }
}
