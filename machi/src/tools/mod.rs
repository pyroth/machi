//! Built-in tools for agents.
//!
//! This module provides a collection of commonly used tools that agents
//! can use out of the box.

mod final_answer;
mod user_input;
mod visit_webpage;
mod web_search;

pub use final_answer::{FinalAnswerArgs, FinalAnswerTool};
pub use user_input::{UserInputArgs, UserInputTool};
pub use visit_webpage::{VisitWebpageArgs, VisitWebpageTool};
pub use web_search::{
    DuckDuckGoSearchTool, SearchEngine, SearchResult, WebSearchArgs, WebSearchTool,
};

use crate::tool::BoxedTool;

/// Get the default tools for agents.
///
/// Returns a vector containing only the `FinalAnswerTool` as it's the essential
/// tool for concluding agent tasks. Other tools like `WebSearchTool`,
/// `VisitWebpageTool`, and `UserInputTool` can be added manually as needed.
#[must_use]
pub fn default_tools() -> Vec<BoxedTool> {
    vec![Box::new(FinalAnswerTool)]
}

/// Get all available built-in tools.
///
/// Returns a vector containing all built-in tools:
/// - `FinalAnswerTool` - for providing final answers
/// - `WebSearchTool` - for web searches
/// - `VisitWebpageTool` - for visiting webpages
/// - `UserInputTool` - for interactive user input
#[must_use]
pub fn all_tools() -> Vec<BoxedTool> {
    vec![
        Box::new(FinalAnswerTool),
        Box::new(WebSearchTool::default()),
        Box::new(VisitWebpageTool::default()),
        Box::new(UserInputTool),
    ]
}

/// Tool names that are available as built-in tools.
pub const BUILTIN_TOOL_NAMES: &[&str] =
    &["final_answer", "web_search", "visit_webpage", "user_input"];
