//! Skills system for extensible agent capabilities.
//!
//! Skills are modular extensions that add new tools and capabilities to the agent.
//! They can be loaded from local directories or remote sources.

mod loader;
mod registry;

pub use loader::{SkillLoader, SkillManifest, SkillSource};
pub use registry::{Skill, SkillInfo, SkillRegistry};
