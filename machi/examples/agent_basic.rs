use machi::client::Nothing;
use machi::completion::Prompt;
use machi::prelude::*;
use machi::providers::ollama::{self, QWEN3};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Create Ollama client
    let client = ollama::Client::from_val(Nothing);

    // Create agent with a single context prompt
    let agent = client
        .agent(QWEN3)
        .preamble("You are a comedian here to entertain the user using humour and jokes.")
        .build();

    // Prompt the agent and print the response
    let response = agent.prompt("Entertain me!").await?;
    println!("{response}");

    Ok(())
}
