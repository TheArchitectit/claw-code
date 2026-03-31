//! Streaming response handler module.
//!
//! This module provides the `StreamHandler` which manages the accumulation
//! of streaming LLM response chunks into complete content blocks.

pub mod events;
pub mod handler;
pub mod state;

pub use events::StreamEvent;
pub use handler::StreamHandler;
