//! Message window — manages conversation history size by pruning old messages.
//!
//! When the conversation exceeds [`AgentConfig::max_messages`], this module
//! removes old messages while preserving two invariants:
//! 1. The system message (index 0) is always kept
//! 2. Cuts only happen at user-turn boundaries to avoid orphaned tool results

use async_openai::types::chat::ChatCompletionRequestMessage;

/// Manages pruning of conversation history to stay within size limits.
pub struct MessageWindow;

impl MessageWindow {
    /// Prune messages to fit within `max`, keeping system message and recent turns.
    ///
    /// Pruning always cuts at a user-message boundary — never mid-conversation —
    /// to ensure tool calls and their results stay paired.
    pub fn prune(messages: &mut Vec<ChatCompletionRequestMessage>, max: usize) {
        if messages.len() <= max {
            return;
        }

        // Always keep system message (index 0)
        let system_len = if Self::is_system_message(&messages[0]) {
            1
        } else {
            0
        };

        // Already within limit?
        if messages.len() <= max {
            return;
        }

        // How many messages can we keep from the tail?
        let target_keep = max.saturating_sub(system_len);
        let prune_start = messages.len() - target_keep;

        // Find nearest user turn at or before prune_start (safety boundary)
        let mut actual_start = prune_start;
        for i in (system_len..prune_start).rev() {
            if Self::is_user_message(&messages[i]) {
                actual_start = i;
                break;
            }
        }

        // Remove messages in [system_len..actual_start)
        if actual_start > system_len {
            messages.drain(system_len..actual_start);
        }
    }

    /// Returns true if the message is a system message.
    pub fn is_system_message(msg: &ChatCompletionRequestMessage) -> bool {
        matches!(msg, ChatCompletionRequestMessage::System(_))
    }

    /// Returns true if the message is a user message.
    pub fn is_user_message(msg: &ChatCompletionRequestMessage) -> bool {
        matches!(msg, ChatCompletionRequestMessage::User(_))
    }
}
