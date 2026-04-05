#[derive(Debug, Clone, Copy)]
pub struct AgentConfig {
    pub tool_max_iterations: usize,
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
    pub fn with_tool_max_iterations(mut self, max: usize) -> Self {
        self.tool_max_iterations = max;
        self
    }

    pub fn with_max_messages(mut self, max: usize) -> Self {
        self.max_messages = max;
        self
    }
}
