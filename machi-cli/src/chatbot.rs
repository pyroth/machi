//! CLI chatbot module for interactive conversations with AI agents.
//!
//! Provides a simple, streaming CLI interface for chatting with Machi agents.

use std::io::{self, Write};

use futures::StreamExt;
use machi::{
    agent::{Agent, MultiTurnStreamItem, Text},
    completion::{
        CompletionError, CompletionModel, GetTokenUsage, PromptError, Usage,
        message::Message,
        streaming::{StreamedAssistantContent, StreamingPrompt},
    },
    core::wasm_compat::WasmCompatSend,
};

/// Configuration for the chatbot.
#[derive(Debug, Clone)]
pub struct ChatBotConfig {
    /// Maximum depth for multi-turn tool calls.
    pub multi_turn_depth: usize,
    /// Whether to display token usage after each response.
    pub show_usage: bool,
    /// System prompt/preamble for the agent.
    pub system_prompt: Option<String>,
}

impl Default for ChatBotConfig {
    fn default() -> Self {
        Self {
            multi_turn_depth: 5,
            show_usage: false,
            system_prompt: None,
        }
    }
}

/// A streaming CLI chatbot powered by Machi agents.
pub struct ChatBot<M>
where
    M: CompletionModel + 'static,
{
    agent: Agent<M>,
    config: ChatBotConfig,
    history: Vec<Message>,
    last_usage: Option<Usage>,
}

impl<M> ChatBot<M>
where
    M: CompletionModel + WasmCompatSend + 'static,
    M::StreamingResponse: GetTokenUsage,
{
    /// Create a new chatbot with the given agent and configuration.
    #[inline]
    pub fn new(agent: Agent<M>, config: ChatBotConfig) -> Self {
        Self {
            agent,
            config,
            history: Vec::new(),
            last_usage: None,
        }
    }

    /// Send a single prompt and stream the response to stdout.
    pub async fn chat(&mut self, prompt: &str) -> Result<String, PromptError> {
        let mut stream = self
            .agent
            .stream_prompt(prompt)
            .with_history(self.history.clone())
            .multi_turn(self.config.multi_turn_depth)
            .await;

        let mut response = String::new();

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(
                    Text { text },
                ))) => {
                    print!("{text}");
                    io::stdout().flush().ok();
                    response.push_str(&text);
                }
                Ok(MultiTurnStreamItem::FinalResponse(final_resp)) => {
                    self.last_usage = Some(final_resp.usage().clone());
                }
                Err(e) => {
                    return Err(PromptError::CompletionError(
                        CompletionError::ResponseError(e.to_string()),
                    ));
                }
                _ => {}
            }
        }

        // Update history
        self.history.push(Message::user(prompt));
        self.history.push(Message::assistant(&response));

        Ok(response)
    }

    /// Run the interactive REPL loop.
    pub async fn run(&mut self) -> Result<(), PromptError> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        println!("Machi CLI Chatbot (type 'exit' or Ctrl+C to quit, 'clear' to reset history)");
        println!();

        loop {
            print!("> ");
            stdout.flush().ok();

            let mut input = String::new();
            if stdin.read_line(&mut input).is_err() {
                continue;
            }

            let input = input.trim();
            if input.is_empty() {
                continue;
            }

            match input {
                "exit" | "quit" => break,
                "clear" => {
                    self.history.clear();
                    println!("History cleared.");
                    continue;
                }
                _ => {}
            }

            println!();
            self.chat(input).await?;
            println!();

            if self.config.show_usage {
                if let Some(usage) = &self.last_usage {
                    println!(
                        "[Tokens: {} in / {} out]",
                        usage.input_tokens, usage.output_tokens
                    );
                }
            }
            println!();
        }

        Ok(())
    }

    /// Clear the conversation history.
    #[inline]
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Get the last token usage.
    #[inline]
    pub fn last_usage(&self) -> Option<&Usage> {
        self.last_usage.as_ref()
    }
}
