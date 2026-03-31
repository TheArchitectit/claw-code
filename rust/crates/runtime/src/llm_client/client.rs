//! LLM client trait definition.
//!
//! This module provides the core trait for LLM client implementations,
//! defining the interface for streaming and complete responses.

use async_trait::async_trait;

use crate::llm_client::stream::LlmStream;
use crate::llm_client::types::{LlmRequest, LlmResponse};
use crate::types::QueryResult;

/// Trait for LLM clients.
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Send a request to the LLM and get a streaming response.
    async fn stream(&self, request: LlmRequest) -> QueryResult<LlmStream>;

    /// Send a request to the LLM and get a complete response.
    async fn complete(&self, request: LlmRequest) -> QueryResult<LlmResponse>;
}

/// Boxed LLM client type.
pub type BoxedLlmClient = Box<dyn LlmClient>;

/// Shared LLM client type.
pub type SharedLlmClient = std::sync::Arc<dyn LlmClient>;
