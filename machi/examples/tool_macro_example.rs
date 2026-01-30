//! Example demonstrating the `#[tool]` macro for creating tools.
//!
//! This example shows how to use the `tool` attribute macro to easily
//! convert functions into tools that can be used with Machi agents.
//!
//! Run with: `cargo run -p machi --example tool_macro_example --features derive`

use machi::tool::{Tool, tool};

/// An addition tool.
#[tool(
    description = "Add two numbers together",
    params(a = "The first number to add", b = "The second number to add"),
    required(a, b)
)]
const fn add(a: i32, b: i32) -> Result<i32, machi::tool::ToolError> {
    Ok(a + b)
}

/// A subtraction tool.
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
    // Initialize tracing for debug output
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    // The macro generates static instances with predictable names:
    // - AddTool, AddArgs, ADD_TOOL
    // - SubTool, SubArgs, SUB_TOOL
    println!("Add tool name: {}", Tool::name(&ADD_TOOL));
    println!("Sub tool name: {}", Tool::name(&SUB_TOOL));

    let add_def = Tool::definition(&ADD_TOOL, String::new()).await;
    let sub_def = Tool::definition(&SUB_TOOL, String::new()).await;
    println!(
        "Add definition:\n{}",
        serde_json::to_string_pretty(&add_def.parameters)?
    );
    println!(
        "Sub definition:\n{}",
        serde_json::to_string_pretty(&sub_def.parameters)?
    );

    let result = Tool::call(&ADD_TOOL, AddArgs { a: 10, b: 20 }).await?;
    println!("10 + 20 = {result}");

    let result = Tool::call(&SUB_TOOL, SubArgs { a: 100, b: 42 }).await?;
    println!("100 - 42 = {result}");

    Ok(())
}
