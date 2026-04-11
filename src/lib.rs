//! Atombot — A Rust-native AI agent framework with tool calling.
//!
//! This crate provides the core components for building AI agents that can
//! use tools to interact with the world. It includes:
//!
//! - [`agent`] — Core agent implementation with OpenAI API client
//! - [`security`] — Security utilities (network sandboxing, path validation)

mod utils;
pub use utils::logger;

pub mod agent;
pub mod security;
