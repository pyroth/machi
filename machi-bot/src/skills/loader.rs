//! Skill loading from various sources.

use super::registry::{Skill, SkillInfo};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, info, warn};

/// Error type for skill loading operations.
#[derive(Debug, thiserror::Error)]
pub enum LoaderError {
    #[error("skill not found: {0}")]
    NotFound(String),
    #[error("invalid manifest: {0}")]
    InvalidManifest(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(#[from] serde_json::Error),
}

/// Result type for skill loading operations.
pub type LoaderResult<T> = Result<T, LoaderError>;

/// Skill manifest file structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    /// Skill identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Description.
    pub description: String,
    /// Version.
    pub version: String,
    /// Author.
    #[serde(default)]
    pub author: Option<String>,
    /// Entry point (relative path to main file).
    #[serde(default = "default_entry")]
    pub entry: String,
    /// Tool definitions.
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
}

fn default_entry() -> String {
    "skill.json".to_string()
}

/// Tool definition in a skill manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name.
    pub name: String,
    /// Tool description.
    pub description: String,
    /// Input parameters schema.
    #[serde(default)]
    pub parameters: serde_json::Value,
}

/// Source for loading skills.
#[derive(Debug, Clone)]
pub enum SkillSource {
    /// Load from local directory.
    Local(PathBuf),
    /// Load from URL.
    Remote(String),
}

/// Skill loader for loading skills from various sources.
#[derive(Debug)]
pub struct SkillLoader {
    /// Base directory for local skills.
    skills_dir: PathBuf,
}

impl SkillLoader {
    /// Create a new skill loader.
    pub fn new(skills_dir: impl Into<PathBuf>) -> Self {
        Self {
            skills_dir: skills_dir.into(),
        }
    }

    /// Get the skills directory.
    #[must_use]
    pub const fn skills_dir(&self) -> &PathBuf {
        &self.skills_dir
    }

    /// List available skills in the skills directory.
    pub async fn list_available(&self) -> LoaderResult<Vec<SkillManifest>> {
        let mut manifests = Vec::new();

        if !self.skills_dir.exists() {
            return Ok(manifests);
        }

        let mut entries = tokio::fs::read_dir(&self.skills_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                let manifest_path = path.join("manifest.json");
                if manifest_path.exists() {
                    match self.load_manifest(&manifest_path).await {
                        Ok(manifest) => manifests.push(manifest),
                        Err(e) => {
                            warn!(
                                path = %manifest_path.display(),
                                error = %e,
                                "failed to load skill manifest"
                            );
                        }
                    }
                }
            }
        }

        Ok(manifests)
    }

    /// Load a skill manifest from a file.
    async fn load_manifest(&self, path: &PathBuf) -> LoaderResult<SkillManifest> {
        let content = tokio::fs::read_to_string(path).await?;
        let manifest: SkillManifest = serde_json::from_str(&content)?;
        debug!(id = %manifest.id, "loaded skill manifest");
        Ok(manifest)
    }

    /// Load a skill by ID from the skills directory.
    pub async fn load(&self, id: &str) -> LoaderResult<Skill> {
        let skill_dir = self.skills_dir.join(id);
        if !skill_dir.exists() {
            return Err(LoaderError::NotFound(id.to_string()));
        }

        let manifest_path = skill_dir.join("manifest.json");
        let manifest = self.load_manifest(&manifest_path).await?;

        // Create skill info from manifest
        let info = SkillInfo {
            id: manifest.id,
            name: manifest.name,
            description: manifest.description,
            version: manifest.version,
            author: manifest.author,
            tools: manifest.tools.iter().map(|t| t.name.clone()).collect(),
            enabled: true,
        };

        // For now, we return an empty tools list
        // In a full implementation, tools would be dynamically loaded
        let skill = Skill {
            info,
            tools: vec![],
        };

        info!(id, "loaded skill");
        Ok(skill)
    }

    /// Load all available skills.
    pub async fn load_all(&self) -> Vec<Skill> {
        let mut skills = Vec::new();

        match self.list_available().await {
            Ok(manifests) => {
                for manifest in manifests {
                    match self.load(&manifest.id).await {
                        Ok(skill) => skills.push(skill),
                        Err(e) => {
                            warn!(id = %manifest.id, error = %e, "failed to load skill");
                        }
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "failed to list available skills");
            }
        }

        skills
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_parsing() {
        let json = r#"{
            "id": "test-skill",
            "name": "Test Skill",
            "description": "A test skill",
            "version": "1.0.0",
            "author": "Test Author",
            "tools": [
                {
                    "name": "test_tool",
                    "description": "A test tool"
                }
            ]
        }"#;

        let manifest: SkillManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.id, "test-skill");
        assert_eq!(manifest.tools.len(), 1);
    }
}
