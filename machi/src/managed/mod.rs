//! Managed agent system for multi-agent collaboration.
//!
//! This module provides the infrastructure for agents to manage and delegate
//! tasks to other agents, following the smolagents architecture pattern.
//!
//! # Architecture
//!
//! A managed agent is an agent that can be called by another (parent) agent
//! as if it were a tool. The parent agent delegates subtasks to managed agents,
//! which execute them independently and return results.
//!
//! # Example
//!
//! ```rust,ignore
//! use machi::prelude::*;
//!
//! // Create a specialized research agent
//! let research_agent = Agent::builder()
//!     .model(model.clone())
//!     .name("researcher")
//!     .description("Expert at finding and summarizing information")
//!     .tool(Box::new(WebSearchTool::new()))
//!     .build();
//!
//! // Create a main agent that can delegate to the research agent
//! let mut main_agent = Agent::builder()
//!     .model(model)
//!     .managed_agent(research_agent)
//!     .build();
//!
//! let result = main_agent.run("Find recent news about Rust programming").await?;
//! ```

mod registry;
mod tool_wrapper;
mod types;

pub use registry::ManagedAgentRegistry;
pub use tool_wrapper::ManagedAgentTool;
pub use types::{
    BoxedManagedAgent, ManagedAgent, ManagedAgentArgs, ManagedAgentInfo, ManagedAgentInput,
    ManagedAgentInputs,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Result;
    use async_trait::async_trait;
    use serde_json::Value;
    use std::collections::HashMap;

    struct MockManagedAgent {
        name: String,
        description: String,
    }

    #[async_trait]
    impl ManagedAgent for MockManagedAgent {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            &self.description
        }

        async fn call(
            &self,
            task: &str,
            _additional_args: Option<HashMap<String, Value>>,
        ) -> Result<String> {
            Ok(format!("Mock agent '{}' processed: {}", self.name, task))
        }
    }

    #[test]
    fn test_managed_agent_info() {
        let info = ManagedAgentInfo::new("researcher", "Finds information");
        assert_eq!(info.name, "researcher");
        assert_eq!(info.output_type, "string");
    }

    #[test]
    fn test_managed_agent_registry() {
        let mut registry = ManagedAgentRegistry::new();

        let agent = MockManagedAgent {
            name: "test_agent".to_string(),
            description: "A test agent".to_string(),
        };

        registry.add(Box::new(agent));
        assert!(registry.contains("test_agent"));
        assert_eq!(registry.len(), 1);
    }

    #[tokio::test]
    async fn test_managed_agent_tool() {
        use crate::tool::DynTool;

        let agent = MockManagedAgent {
            name: "helper".to_string(),
            description: "Helps with tasks".to_string(),
        };

        let tool = ManagedAgentTool::new(Box::new(agent));
        assert_eq!(tool.name(), "helper");

        let args = serde_json::json!({
            "task": "Do something"
        });

        let result = tool
            .call_json(args)
            .await
            .expect("tool call should succeed");
        assert!(
            result
                .as_str()
                .expect("result should be a string")
                .contains("Do something")
        );
    }
}
