use async_openai::types::chat::ChatCompletionRequestMessage;

/// Message window that keeps recent messages and prunes old ones at legal boundaries.
///
/// A "legal" boundary is at a user turn - we never cut in the middle of a
/// tool call / tool result exchange to avoid orphaned tool results.
pub struct MessageWindow;

impl MessageWindow {
    /// Prune messages to fit within max limit, keeping system message and recent turns.
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

        // If we're already within limit even after removing everything before system, nothing to do
        if messages.len() <= max {
            return;
        }

        // Find the starting point: we want to keep the last (max - system_len) messages
        // but we must align to a user turn boundary
        let target_keep = max.saturating_sub(system_len);
        let prune_start = messages.len() - target_keep;

        // Find the nearest user turn at or before prune_start
        let mut actual_start = prune_start;
        for i in (system_len..prune_start).rev() {
            if Self::is_user_message(&messages[i]) {
                actual_start = i;
                break;
            }
        }

        // Remove messages from [system_len..actual_start)
        if actual_start > system_len {
            messages.drain(system_len..actual_start);
        }
    }

    pub fn is_system_message(msg: &ChatCompletionRequestMessage) -> bool {
        matches!(msg, ChatCompletionRequestMessage::System(_))
    }

    pub fn is_user_message(msg: &ChatCompletionRequestMessage) -> bool {
        matches!(msg, ChatCompletionRequestMessage::User(_))
    }
}
