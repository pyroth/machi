//! Example demonstrating how to use the `#[tool]` macro with an Ollama agent.
//!
//! Run with: `cargo run -p machi --example agent_with_tools_macro --features derive`

use anyhow::Result;
use machi::client::Nothing;
use machi::completion::Prompt;
use machi::prelude::*;
use machi::providers;
use machi::tool::{ToolDyn, tool};

/// An addition tool.
#[allow(clippy::unnecessary_wraps)]
#[tool(
    description = "Add two numbers together",
    params(a = "The first number to add", b = "The second number to add"),
    required(a, b)
)]
const fn add(a: i32, b: i32) -> Result<i32, machi::tool::ToolError> {
    Ok(a + b)
}

/// A subtraction tool.
#[allow(clippy::unnecessary_wraps)]
#[tool(
    description = "Sub one number from another",
    params(a = "The number to subtract from", b = "The number to subtract"),
    required(a, b)
)]
const fn sub(a: i32, b: i32) -> Result<i32, machi::tool::ToolError> {
    Ok(a - b)
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_target(false)
        .init();

    // Create Ollama client (defaults to http://localhost:11434)
    let ollama_client: providers::ollama::Client = providers::ollama::Client::builder()
        .api_key(Nothing)
        .build()
        .expect("Failed to create Ollama client");

    // The #[tool] macro generates static instances: ADD_TOOL and SUB_TOOL
    let tools: Vec<Box<dyn ToolDyn>> = vec![Box::new(AddTool), Box::new(SubTool)];

    // Create agent with a single context prompt and two tools
    // Using LLAMA3_2 model which supports tool calling
    let calculator_agent = ollama_client
        .agent(providers::ollama::QWEN3)
        .preamble("You are a calculator here to help the user perform arithmetic operations. Use the tools provided to answer the user's question.")
        .tools(tools)
        .max_tokens(1024)
        .build();

    // Prompt the agent and print the response
    println!("Calculate 2 - 5");
    println!(
        "Ollama Calculator Agent: {}",
        calculator_agent.prompt("Calculate 2 - 5").await?
    );

    Ok(())
}
