//! LLM client abstraction for the QueryEngine.
//!
//! This module provides traits and types for interacting with LLM APIs
//! in a streaming fashion, supporting the tool-call loop pattern.

pub mod client;
pub mod mock;
pub mod stream;
pub mod types;

// Re-export all public types for backward compatibility
pub use client::{BoxedLlmClient, LlmClient, SharedLlmClient};
pub use mock::{MockLlmClient, MockResponse};
pub use stream::{LlmStream, LlmStreamChunk, StopReason};
pub use types::{LlmRequest, LlmResponse};

#[cfg(test)]
pub mod tests;
