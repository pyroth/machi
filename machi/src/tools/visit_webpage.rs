//! Tool for visiting and reading webpage content.
//!
//! Ported from smolagents' VisitWebpageTool implementation.

use crate::tool::{Tool, ToolError};
use async_trait::async_trait;
use regex::Regex;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Write;
use std::sync::LazyLock;

/// Tool for visiting a webpage and extracting its content as markdown.
/// Ported from smolagents' VisitWebpageTool.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct VisitWebpageTool {
    /// Maximum output length in characters.
    pub max_output_length: usize,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
}

impl Default for VisitWebpageTool {
    fn default() -> Self {
        Self {
            max_output_length: 40000,
            timeout_secs: 20,
        }
    }
}

/// Arguments for visiting a webpage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VisitWebpageArgs {
    /// The URL of the webpage to visit.
    pub url: String,
}

// Pre-compiled regex for cleaning up multiple newlines
static MULTILINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\n{3,}").expect("valid regex"));

impl VisitWebpageTool {
    /// Create a new webpage visitor tool.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum output length.
    #[must_use]
    pub const fn with_max_output_length(mut self, max: usize) -> Self {
        self.max_output_length = max;
        self
    }

    /// Set request timeout.
    #[must_use]
    pub const fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Truncate content to max length (same as smolagents' _truncate_content).
    /// Uses char boundary to avoid splitting multi-byte characters.
    fn truncate_content(&self, content: &str) -> String {
        if content.len() <= self.max_output_length {
            content.to_string()
        } else {
            // Find a valid char boundary to avoid splitting multi-byte characters
            let truncate_at = content
                .char_indices()
                .take_while(|(i, _)| *i < self.max_output_length)
                .last()
                .map_or(0, |(i, c)| i + c.len_utf8());
            format!(
                "{}\n..._This content has been truncated to stay below {} characters_...\n",
                &content[..truncate_at],
                self.max_output_length
            )
        }
    }

    /// Convert HTML to markdown using DOM parsing (similar to markdownify).
    fn html_to_markdown(html: &str) -> String {
        let document = Html::parse_document(html);
        let mut output = String::new();

        // Remove script, style, noscript, and other non-content elements
        let body_selector = Selector::parse("body").ok();
        let root = body_selector
            .as_ref()
            .and_then(|s| document.select(s).next())
            .map_or_else(|| html.to_string(), |el| el.html());

        // Parse the body content
        let body_doc = Html::parse_fragment(&root);

        // Process elements recursively
        Self::process_node(&body_doc.root_element(), &mut output, 0);

        // Clean up the output
        let cleaned = MULTILINE_RE.replace_all(&output, "\n\n");
        cleaned.trim().to_string()
    }

    /// Process a DOM node and convert to markdown.
    /// Note: `_depth` is preserved for potential future use (e.g., nested list indentation).
    fn process_node(element: &scraper::ElementRef<'_>, output: &mut String, _depth: usize) {
        use scraper::Node;

        for child in element.children() {
            match child.value() {
                Node::Text(text) => {
                    let text_str = text.text.trim();
                    if !text_str.is_empty() {
                        output.push_str(text_str);
                        output.push(' ');
                    }
                }
                Node::Element(el) => {
                    let tag = el.name();
                    let child_ref = scraper::ElementRef::wrap(child);

                    if let Some(child_el) = child_ref {
                        match tag {
                            // Skip non-content elements
                            "script" | "style" | "noscript" | "iframe" | "svg" | "path" => {}

                            // Headings
                            "h1" => {
                                output.push_str("\n\n# ");
                                Self::process_node(&child_el, output, 0);
                                output.push_str("\n\n");
                            }
                            "h2" => {
                                output.push_str("\n\n## ");
                                Self::process_node(&child_el, output, 0);
                                output.push_str("\n\n");
                            }
                            "h3" => {
                                output.push_str("\n\n### ");
                                Self::process_node(&child_el, output, 0);
                                output.push_str("\n\n");
                            }
                            "h4" => {
                                output.push_str("\n\n#### ");
                                Self::process_node(&child_el, output, 0);
                                output.push_str("\n\n");
                            }
                            "h5" => {
                                output.push_str("\n\n##### ");
                                Self::process_node(&child_el, output, 0);
                                output.push_str("\n\n");
                            }
                            "h6" => {
                                output.push_str("\n\n###### ");
                                Self::process_node(&child_el, output, 0);
                                output.push_str("\n\n");
                            }

                            // Paragraphs, divs, and tables (block-level elements)
                            "p" | "div" | "section" | "article" | "main" | "header" | "footer"
                            | "table" => {
                                output.push_str("\n\n");
                                Self::process_node(&child_el, output, 0);
                                output.push_str("\n\n");
                            }

                            // Line breaks
                            "br" => {
                                output.push('\n');
                            }
                            "hr" => {
                                output.push_str("\n\n---\n\n");
                            }

                            // Lists
                            "ul" | "ol" => {
                                output.push('\n');
                                Self::process_node(&child_el, output, 1);
                                output.push('\n');
                            }
                            "li" => {
                                output.push_str("\n- ");
                                Self::process_node(&child_el, output, 0);
                            }

                            // Links
                            "a" => {
                                let href = el.attr("href").unwrap_or("#");
                                let mut link_text = String::new();
                                Self::process_node(&child_el, &mut link_text, 0);
                                let link_text = link_text.trim();
                                if !link_text.is_empty() && href != "#" {
                                    let _ = write!(output, "[{link_text}]({href})");
                                } else if !link_text.is_empty() {
                                    output.push_str(link_text);
                                }
                            }

                            // Images
                            "img" => {
                                let alt = el.attr("alt").unwrap_or("image");
                                let src = el.attr("src").unwrap_or("");
                                if !src.is_empty() {
                                    let _ = write!(output, "![{alt}]({src})");
                                }
                            }

                            // Text formatting
                            "strong" | "b" => {
                                output.push_str("**");
                                Self::process_node(&child_el, output, 0);
                                output.push_str("**");
                            }
                            "em" | "i" => {
                                output.push('*');
                                Self::process_node(&child_el, output, 0);
                                output.push('*');
                            }
                            "code" => {
                                output.push('`');
                                Self::process_node(&child_el, output, 0);
                                output.push('`');
                            }
                            "pre" => {
                                output.push_str("\n\n```\n");
                                Self::process_node(&child_el, output, 0);
                                output.push_str("\n```\n\n");
                            }

                            // Blockquotes
                            "blockquote" => {
                                output.push_str("\n\n> ");
                                let mut quote_text = String::new();
                                Self::process_node(&child_el, &mut quote_text, 0);
                                output.push_str(&quote_text.replace('\n', "\n> "));
                                output.push_str("\n\n");
                            }

                            // Table rows
                            "tr" => {
                                output.push_str("| ");
                                Self::process_node(&child_el, output, 0);
                                output.push('\n');
                            }
                            "th" | "td" => {
                                Self::process_node(&child_el, output, 0);
                                output.push_str(" | ");
                            }

                            // Default: just process children (includes thead, tbody, tfoot, etc.)
                            _ => {
                                Self::process_node(&child_el, output, 0);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

#[async_trait]
impl Tool for VisitWebpageTool {
    const NAME: &'static str = "visit_webpage";
    type Args = VisitWebpageArgs;
    type Output = String;
    type Error = ToolError;

    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn description(&self) -> String {
        "Visits a webpage at the given URL and reads its content as a markdown string. Use this to browse webpages.".to_string()
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "format": "uri",
                    "description": "The URL of the webpage to visit (must be a valid HTTP/HTTPS URL)"
                }
            },
            "required": ["url"]
        })
    }

    fn output_type(&self) -> &'static str {
        "string"
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Validate URL format
        if !args.url.starts_with("http://") && !args.url.starts_with("https://") {
            return Err(ToolError::InvalidArguments(
                "URL must start with http:// or https://".to_string(),
            ));
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(self.timeout_secs))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

        let response = client.get(&args.url).send().await.map_err(|e| {
            if e.is_timeout() {
                ToolError::ExecutionError("Request timed out. Please try again later.".to_string())
            } else {
                ToolError::ExecutionError(format!("Error fetching webpage: {e}"))
            }
        })?;

        if !response.status().is_success() {
            return Err(ToolError::ExecutionError(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        let html = response
            .text()
            .await
            .map_err(|e| ToolError::ExecutionError(format!("Failed to read response: {e}")))?;

        let markdown = Self::html_to_markdown(&html);
        Ok(self.truncate_content(&markdown))
    }
}
