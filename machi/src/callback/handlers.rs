//! Callback handler types and traits.

use super::context::CallbackContext;
use crate::memory::MemoryStep;
use std::any::TypeId;
use std::cmp::Ordering;
use std::sync::Arc;

/// Priority level for callback execution order.
///
/// Callbacks with higher priority execute first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Priority(pub i32);

impl Priority {
    /// Highest priority - executes first.
    pub const HIGHEST: Self = Self(1000);
    /// High priority.
    pub const HIGH: Self = Self(100);
    /// Normal/default priority.
    pub const NORMAL: Self = Self(0);
    /// Low priority.
    pub const LOW: Self = Self(-100);
    /// Lowest priority - executes last.
    pub const LOWEST: Self = Self(-1000);
}

impl PartialOrd for Priority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Priority {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher value = higher priority = should come first
        other.0.cmp(&self.0)
    }
}

/// Type alias for synchronous callback function.
pub type CallbackFn<S> = dyn Fn(&S, &CallbackContext) + Send + Sync;

/// Type alias for boxed synchronous callback.
pub type BoxedCallback = Box<dyn Fn(&dyn MemoryStep, &CallbackContext) + Send + Sync>;

// Async callbacks are implemented in async_registry.rs with owned step data
// to avoid lifetime issues with borrowed references in async contexts.

/// Internal handler wrapper that stores callback with metadata.
pub struct CallbackHandler {
    /// The callback function.
    pub callback: Arc<BoxedCallback>,
    /// Priority for ordering.
    pub priority: Priority,
    /// Target type ID (None = any step).
    pub target_type: Option<TypeId>,
    /// Optional name for debugging.
    pub name: Option<String>,
}

impl CallbackHandler {
    /// Create a new handler for a specific step type.
    pub fn new<S, F>(callback: F, priority: Priority) -> Self
    where
        S: MemoryStep + 'static,
        F: Fn(&S, &CallbackContext) + Send + Sync + 'static,
    {
        let wrapped: BoxedCallback = Box::new(move |step, ctx| {
            if let Some(typed_step) = step.as_any().downcast_ref::<S>() {
                callback(typed_step, ctx);
            }
        });

        Self {
            callback: Arc::new(wrapped),
            priority,
            target_type: Some(TypeId::of::<S>()),
            name: None,
        }
    }

    /// Create a new handler for any step type.
    pub fn any<F>(callback: F, priority: Priority) -> Self
    where
        F: Fn(&dyn MemoryStep, &CallbackContext) + Send + Sync + 'static,
    {
        Self {
            callback: Arc::new(Box::new(callback)),
            priority,
            target_type: None,
            name: None,
        }
    }

    /// Set a name for this handler (useful for debugging).
    #[must_use]
    #[allow(dead_code)]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Check if this handler matches the given step type.
    #[allow(dead_code)]
    pub fn matches(&self, step: &dyn MemoryStep) -> bool {
        match self.target_type {
            None => true, // Matches any
            Some(type_id) => step.as_any().type_id() == type_id,
        }
    }

    /// Invoke the callback.
    pub fn invoke(&self, step: &dyn MemoryStep, ctx: &CallbackContext) {
        (self.callback)(step, ctx);
    }
}

impl std::fmt::Debug for CallbackHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CallbackHandler")
            .field("priority", &self.priority)
            .field("target_type", &self.target_type)
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

// Note: Async callbacks are complex due to lifetime issues with borrowed step data.
// For now, we provide sync callbacks only. Async support may be added in a future version
// using a different approach (e.g., cloning step data or using channels).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_ordering() {
        let mut priorities = [
            Priority::LOW,
            Priority::HIGHEST,
            Priority::NORMAL,
            Priority::HIGH,
            Priority::LOWEST,
        ];
        priorities.sort();

        assert_eq!(priorities[0], Priority::HIGHEST);
        assert_eq!(priorities[1], Priority::HIGH);
        assert_eq!(priorities[2], Priority::NORMAL);
        assert_eq!(priorities[3], Priority::LOW);
        assert_eq!(priorities[4], Priority::LOWEST);
    }
}
