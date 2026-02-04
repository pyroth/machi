//! Registry for managing multiple managed agents.

use std::collections::HashMap;
use std::sync::Arc;

use crate::error::{AgentError, Result};
use crate::tool::BoxedTool;

use super::tool_wrapper::{ManagedAgentTool, ManagedAgentToolClone};
use super::types::{BoxedManagedAgent, ManagedAgentInfo};

/// A collection of managed agents.
#[derive(Default)]
pub struct ManagedAgentRegistry {
    /// Map of agent names to their tool wrappers.
    agents: HashMap<String, ManagedAgentTool>,
}

impl ManagedAgentRegistry {
    /// Create a new empty registry.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a managed agent to the registry.
    ///
    /// # Panics
    ///
    /// Panics if an agent with the same name already exists.
    #[track_caller]
    pub fn add(&mut self, agent: BoxedManagedAgent) {
        let name = agent.name().to_string();
        assert!(
            !self.agents.contains_key(&name),
            "Managed agent with name '{name}' already exists"
        );
        self.agents.insert(name, ManagedAgentTool::new(agent));
    }

    /// Try to add a managed agent, returning an error if the name is taken.
    pub fn try_add(&mut self, agent: BoxedManagedAgent) -> Result<()> {
        use std::collections::hash_map::Entry;

        let name = agent.name().to_string();
        match self.agents.entry(name) {
            Entry::Occupied(e) => Err(AgentError::configuration(format!(
                "Managed agent with name '{}' already exists",
                e.key()
            ))),
            Entry::Vacant(e) => {
                e.insert(ManagedAgentTool::new(agent));
                Ok(())
            }
        }
    }

    /// Get a managed agent by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&ManagedAgentTool> {
        self.agents.get(name)
    }

    /// Get all managed agents as boxed tools.
    #[must_use]
    pub fn as_tools(&self) -> Vec<BoxedTool> {
        self.agents
            .values()
            .map(|agent| -> BoxedTool {
                Box::new(ManagedAgentToolClone {
                    agent: Arc::clone(&agent.clone_arc()),
                    info: agent.agent_info().clone(),
                })
            })
            .collect()
    }

    /// Get info for all managed agents (for prompt generation).
    #[must_use]
    pub fn infos(&self) -> HashMap<String, ManagedAgentInfo> {
        self.agents
            .iter()
            .map(|(name, agent)| (name.clone(), agent.agent_info().clone()))
            .collect()
    }

    /// Get the names of all managed agents.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.agents.keys().map(String::as_str).collect()
    }

    /// Check if a managed agent with the given name exists.
    #[inline]
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.agents.contains_key(name)
    }

    /// Get the number of managed agents.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    /// Check if the registry is empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }

    /// Iterate over all managed agents.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &ManagedAgentTool)> {
        self.agents.iter().map(|(k, v)| (k.as_str(), v))
    }
}

impl std::fmt::Debug for ManagedAgentRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedAgentRegistry")
            .field("agents", &self.names())
            .finish()
    }
}
