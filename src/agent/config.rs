//! Agent configuration options.

/// Configuration for agent behavior.
#[derive(Debug, Clone, Copy)]
pub struct AgentConfig {
    /// Maximum number of tool-call iterations before giving up.
    /// This prevents infinite loops when the model keeps calling tools.
    pub tool_max_iterations: usize,
    /// Maximum number of messages to keep in conversation history.
    /// Old messages are pruned when this limit is exceeded.
    pub max_messages: usize,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            tool_max_iterations: 10,
            max_messages: 40,
        }
    }
}

impl AgentConfig {
    /// Set the maximum number of tool iterations.
    pub fn with_tool_max_iterations(mut self, max: usize) -> Self {
        self.tool_max_iterations = max;
        self
    }

    /// Set the maximum number of messages to retain.
    pub fn with_max_messages(mut self, max: usize) -> Self {
        self.max_messages = max;
        self
    }
}
