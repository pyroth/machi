//! Basic agent example.
//!
//! This example demonstrates how to create a simple agent with
//! an embedded wallet and Ethereum chain support.
//!
//! Run with: `cargo run --example basic_agent`
//!
//! Note: Requires OPENAI_API_KEY environment variable to be set.

use machi::backend::rig::RigBackend;
use machi::chain::ethereum::Ethereum;
use machi::{AgentBuilder, AgentWallet};
use rig::client::{CompletionClient, ProviderClient};
use rig::providers::openai;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Machi Web3 Agent Demo ===\n");

    // Create OpenAI client and agent
    let openai_client = openai::Client::from_env();
    let gpt4 = openai_client
        .agent("gpt-4o-mini")
        .preamble(
            "You are a helpful Web3 assistant with access to a cryptocurrency wallet. \
             You can check wallet addresses and balances. When asked about wallet info, \
             use the available tools to get accurate information.",
        )
        .build();

    // Wrap rig agent in our backend adapter
    let backend = RigBackend::new(gpt4);

    // Create Ethereum chain adapter (using public RPC)
    let chain = Ethereum::sepolia("https://ethereum-sepolia-rpc.publicnode.com");

    // Generate a new wallet for the agent
    let wallet = AgentWallet::generate(12, None)?;
    println!("🔐 Generated new wallet");
    println!("   Mnemonic: {}", wallet.mnemonic());

    // Build the agent
    let agent = AgentBuilder::new()
        .backend(backend)
        .chain(chain)
        .wallet(wallet)
        .build()?;

    // Get the agent's Ethereum address
    let address = agent.address()?;
    println!("   Address: {address}\n");

    // Example 1: Simple chat (no tools)
    println!("--- Example 1: Simple Chat ---");
    let response = agent.chat("What is Ethereum?").await?;
    println!("User: What is Ethereum?");
    println!("Agent: {}\n", truncate(&response, 200));

    // Example 2: Using agent.run() with tool execution
    println!("--- Example 2: Agent Run with Tools ---");
    println!("User: What is my wallet address?");
    let response = agent.run("What is my wallet address?").await?;
    println!("Agent: {}\n", response);

    // Example 3: Check balance through agent
    println!("--- Example 3: Balance Query ---");
    println!("User: Check my wallet balance");
    let response = agent.run("Check my wallet balance on Ethereum").await?;
    println!("Agent: {}\n", response);

    // Example 4: Direct API usage
    println!("--- Example 4: Direct API ---");
    match agent.balance().await {
        Ok(balance) => println!("Direct balance query: {} wei", balance),
        Err(e) => println!("Balance query failed: {e}"),
    }

    println!("\n=== Demo Complete ===");
    Ok(())
}

/// Truncate a string for display.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}
