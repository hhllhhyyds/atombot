//! Security utilities for Atombot.
//!
//! Provides network sandboxing and URL validation to prevent
//! SSRF attacks and access to internal/private network resources.

pub mod allowed_dir;
pub mod network;
