//! Anthropic API client module.
//!
//! This module provides a concrete implementation of the `LlmClient` trait
//! that connects to the Anthropic API (Claude).

pub mod client;
pub mod config;
pub mod error;

pub use config::{AnthropicClient, AnthropicConfig};
pub use error::AnthropicError;
