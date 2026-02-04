//! Telemetry example demonstrating automatic metrics collection.
//!
//! The Agent automatically collects telemetry during execution.
//! Configure a tracing subscriber to see events in the console.
//! For OpenTelemetry export, add `tracing-opentelemetry` layer.
//!
//! ```bash
//! ollama pull qwen3
//! cargo run --example agent_telemetry
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr, clippy::unused_async)]

use machi::prelude::*;

/// Adds two numbers.
#[machi::tool]
async fn add(a: i64, b: i64) -> std::result::Result<i64, ToolError> {
    Ok(a + b)
}

/// Multiplies two numbers.
#[machi::tool]
async fn multiply(a: i64, b: i64) -> std::result::Result<i64, ToolError> {
    Ok(a * b)
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let model = OllamaClient::new().completion_model("qwen3");
    let mut agent = Agent::builder()
        .model(model)
        .tool(Box::new(Add))
        .tool(Box::new(Multiply))
        .max_steps(10)
        .build();

    println!("Running agent with telemetry...\n");

    // Run the task - telemetry is collected automatically
    let result = agent.run("Calculate (5 + 3) * 2").await;

    // Get metrics from the agent
    let metrics = agent.metrics();
    println!("\n{metrics}");

    // Show result
    match result {
        Ok(answer) => println!("Final Answer: {answer}"),
        Err(e) => println!("Error: {e}"),
    }

    Ok(())
}
