//! Tool execution policy with human confirmation example.
//!
//! Demonstrates how to require user confirmation before executing sensitive tools.
//!
//! ```bash
//! ollama pull qwen3
//! cargo run --example agent_confirmation
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr, clippy::unused_async)]

use async_trait::async_trait;
use machi::prelude::*;
use std::io::{self, Write};

/// A sensitive tool that deletes a file (simulated).
#[machi::tool]
async fn delete_file(path: String) -> ToolResult<String> {
    // Simulated deletion - in real use, this would actually delete files
    Ok(format!("File '{path}' deleted successfully"))
}

/// CLI confirmation handler that prompts the user in terminal.
struct CliConfirmationHandler;

#[async_trait]
impl ConfirmationHandler for CliConfirmationHandler {
    async fn confirm(&self, request: &ToolConfirmationRequest) -> ToolConfirmationResponse {
        println!("\n{}", request.description);
        print!("Allow? [y/n/a(ll)]: ");
        io::stdout().flush().expect("Failed to flush stdout");

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read user input");

        match input.trim().to_lowercase().as_str() {
            "y" | "yes" => ToolConfirmationResponse::Approved,
            "a" | "all" => ToolConfirmationResponse::ApproveAll,
            _ => ToolConfirmationResponse::Denied,
        }
    }
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let model = OllamaClient::new().completion_model("qwen3");

    let mut agent = Agent::builder()
        .model(model)
        .tool(Box::new(DeleteFile))
        .tool_policy("delete_file", ToolExecutionPolicy::RequireConfirmation)
        .confirmation_handler(Box::new(CliConfirmationHandler))
        .max_steps(5)
        .build();

    let task = "Delete the file named 'test.txt'";
    println!("Task: {task}\n");

    match agent.run(task).await {
        Ok(result) => println!("\nResult: {result}"),
        Err(e) => eprintln!("\nError: {e}"),
    }

    Ok(())
}
