//! Builder for coordinator handle.

use std::sync::Arc;

use super::integration::CoordinatorHandle;
use super::message_bus::Coordinator;

/// Builder for coordinator handle.
pub struct CoordinatorHandleBuilder {
    coordinator: Option<Arc<Coordinator>>,
}

impl CoordinatorHandleBuilder {
    /// Create a new builder.
    #[must_use]
    pub fn new() -> Self {
        Self { coordinator: None }
    }

    /// Set the coordinator.
    #[must_use]
    pub fn with_coordinator(mut self, coordinator: Arc<Coordinator>) -> Self {
        self.coordinator = Some(coordinator);
        self
    }

    /// Build the coordinator handle.
    #[must_use]
    pub fn build(self) -> CoordinatorHandle {
        match self.coordinator {
            Some(coordinator) => CoordinatorHandle::new(coordinator),
            None => CoordinatorHandle::new_without_agent(),
        }
    }
}

impl Default for CoordinatorHandleBuilder {
    fn default() -> Self {
        Self::new()
    }
}
