//! Machi CLI - Interactive AI chatbot powered by Machi framework.

use clap::{Parser, ValueEnum};
use machi::{
    client::{CompletionClient, ProviderClient},
    providers::{anthropic, ollama, openai, xai},
};
use machi_cli::{ChatBot, ChatBotConfig};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// Supported AI providers.
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum Provider {
    /// OpenAI (GPT-4, etc.)
    #[default]
    Openai,
    /// Anthropic (Claude)
    Anthropic,
    /// Ollama (local models)
    Ollama,
    /// xAI (Grok)
    Xai,
}

/// Machi CLI - Interactive AI chatbot
#[derive(Parser, Debug)]
#[command(name = "machi")]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// AI provider to use
    #[arg(short, long, value_enum, default_value_t = Provider::Openai)]
    provider: Provider,

    /// Model name (provider-specific, uses default if not specified)
    #[arg(short, long)]
    model: Option<String>,

    /// System prompt for the agent
    #[arg(short, long)]
    system: Option<String>,

    /// Maximum multi-turn depth for tool calls
    #[arg(long, default_value_t = 5)]
    max_turns: usize,

    /// Show token usage after each response
    #[arg(long)]
    usage: bool,

    /// Ollama server URL (only for ollama provider)
    #[arg(long, env = "OLLAMA_HOST", default_value = "http://localhost:11434")]
    ollama_url: String,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

fn init_tracing(verbose: bool) {
    let filter = if verbose {
        EnvFilter::new("machi=debug,machi_cli=debug")
    } else {
        EnvFilter::new("machi=warn,machi_cli=info")
    };

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false))
        .with(filter)
        .init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    init_tracing(args.verbose);

    let config = ChatBotConfig {
        multi_turn_depth: args.max_turns,
        show_usage: args.usage,
        system_prompt: args.system.clone(),
    };

    let preamble = args
        .system
        .as_deref()
        .unwrap_or("You are a helpful AI assistant.");

    match args.provider {
        Provider::Openai => {
            let client = openai::Client::from_env();
            let model = args
                .model
                .as_deref()
                .unwrap_or(openai::completion::GPT_4O_MINI);

            let agent = client.agent(model).preamble(preamble).build();

            let mut chatbot = ChatBot::new(agent, config);
            chatbot.run().await?;
        }
        Provider::Anthropic => {
            let client = anthropic::Client::from_env();
            let model = args
                .model
                .as_deref()
                .unwrap_or(anthropic::completion::CLAUDE_3_5_SONNET);

            let agent = client.agent(model).preamble(preamble).build();

            let mut chatbot = ChatBot::new(agent, config);
            chatbot.run().await?;
        }
        Provider::Ollama => {
            let client: ollama::Client = ollama::Client::builder()
                .api_key(machi::client::Nothing)
                .base_url(&args.ollama_url)
                .build()?;
            let model = args.model.as_deref().unwrap_or("llama3.2");

            let agent = client.agent(model).preamble(preamble).build();

            let mut chatbot = ChatBot::new(agent, config);
            chatbot.run().await?;
        }
        Provider::Xai => {
            let client = xai::Client::from_env();
            let model = args
                .model
                .as_deref()
                .unwrap_or(xai::completion::GROK_3_MINI_FAST);

            let agent = client.agent(model).preamble(preamble).build();

            let mut chatbot = ChatBot::new(agent, config);
            chatbot.run().await?;
        }
    }

    Ok(())
}
