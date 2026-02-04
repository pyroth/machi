//! Skill registry for managing loaded skills.

use machi::prelude::BoxedTool;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Information about a skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    /// Unique skill identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Description of what the skill does.
    pub description: String,
    /// Version string.
    pub version: String,
    /// Author information.
    pub author: Option<String>,
    /// List of tool names provided by this skill.
    pub tools: Vec<String>,
    /// Whether the skill is currently enabled.
    pub enabled: bool,
}

/// A loaded skill with its tools.
pub struct Skill {
    /// Skill metadata.
    pub info: SkillInfo,
    /// Tools provided by this skill.
    pub tools: Vec<BoxedTool>,
}

impl std::fmt::Debug for Skill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Skill")
            .field("info", &self.info)
            .field("tools_count", &self.tools.len())
            .finish()
    }
}

/// Registry for managing loaded skills.
#[derive(Debug, Default)]
pub struct SkillRegistry {
    skills: Arc<RwLock<HashMap<String, Skill>>>,
}

impl SkillRegistry {
    /// Create a new skill registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a skill.
    pub async fn register(&self, skill: Skill) {
        let id = skill.info.id.clone();
        let mut skills = self.skills.write().await;
        skills.insert(id, skill);
    }

    /// Unregister a skill by ID.
    pub async fn unregister(&self, id: &str) -> Option<Skill> {
        let mut skills = self.skills.write().await;
        skills.remove(id)
    }

    /// Get a skill by ID.
    pub async fn get(&self, id: &str) -> Option<SkillInfo> {
        let skills = self.skills.read().await;
        skills.get(id).map(|s| s.info.clone())
    }

    /// List all registered skills.
    pub async fn list(&self) -> Vec<SkillInfo> {
        let skills = self.skills.read().await;
        skills.values().map(|s| s.info.clone()).collect()
    }

    /// Get tool names from enabled skills.
    pub async fn get_tool_names(&self) -> Vec<String> {
        let skills = self.skills.read().await;
        skills
            .values()
            .filter(|s| s.info.enabled)
            .flat_map(|s| s.info.tools.iter().cloned())
            .collect()
    }

    /// Enable a skill.
    pub async fn enable(&self, id: &str) -> bool {
        let mut skills = self.skills.write().await;
        if let Some(skill) = skills.get_mut(id) {
            skill.info.enabled = true;
            true
        } else {
            false
        }
    }

    /// Disable a skill.
    pub async fn disable(&self, id: &str) -> bool {
        let mut skills = self.skills.write().await;
        if let Some(skill) = skills.get_mut(id) {
            skill.info.enabled = false;
            true
        } else {
            false
        }
    }

    /// Get the count of registered skills.
    pub async fn count(&self) -> usize {
        let skills = self.skills.read().await;
        skills.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_skill_registry() {
        let registry = SkillRegistry::new();

        let skill = Skill {
            info: SkillInfo {
                id: "test".to_string(),
                name: "Test Skill".to_string(),
                description: "A test skill".to_string(),
                version: "1.0.0".to_string(),
                author: Some("Test".to_string()),
                tools: vec!["test_tool".to_string()],
                enabled: true,
            },
            tools: vec![],
        };

        registry.register(skill).await;

        let skills = registry.list().await;
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "Test Skill");

        // Disable
        registry.disable("test").await;
        let info = registry.get("test").await.unwrap();
        assert!(!info.enabled);

        // Unregister
        registry.unregister("test").await;
        assert_eq!(registry.count().await, 0);
    }
}
