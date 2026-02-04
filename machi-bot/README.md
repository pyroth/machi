# Machi Bot

A personal AI assistant framework with multi-channel support, built on the [Machi](../machi) agent framework.

## Features

- **Multi-Channel Support**: CLI, Telegram (more coming)
- **Async Message Bus**: Decoupled channel-agent communication
- **Session Management**: Persistent conversation history
- **Configurable**: JSON config with environment variable overrides
- **Extensible**: Skills system for adding custom tools
- **Voice Transcription**: Groq Whisper integration

## Quick Start

```bash
# Initialize configuration
machi-bot init

# Start interactive CLI chat
machi-bot chat

# Run as a gateway (all channels)
machi-bot gateway
```

## Installation

```bash
cargo install machi-bot
```

Or build from source:

```bash
cargo build --release --features telegram
```

## Configuration

Configuration is stored in `~/.machi-bot/config.json`:

```json
{
  "providers": {
    "anthropic": {
      "apiKey": "sk-..."
    }
  },
  "agents": {
    "defaults": {
      "model": "anthropic/claude-sonnet-4",
      "maxIterations": 20
    }
  },
  "channels": {
    "telegram": {
      "enabled": true,
      "allowFrom": ["123456789"]
    }
  }
}
```

### Environment Variables

- `ANTHROPIC_API_KEY` - Anthropic API key
- `OPENAI_API_KEY` - OpenAI API key
- `TELEGRAM_BOT_TOKEN` - Telegram bot token
- `GROQ_API_KEY` - Groq API key (for transcription)

## Architecture

```text
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Telegram  │────▶│             │────▶│   Agent     │
│   Channel   │     │  Message    │     │   Loop      │
└─────────────┘     │    Bus      │     └─────────────┘
                    │             │            │
┌─────────────┐     │             │            ▼
│    CLI      │────▶│             │     ┌─────────────┐
│   Channel   │     └─────────────┘     │   Session   │
└─────────────┘                         │   Manager   │
                                        └─────────────┘
```

### Core Modules

- **`bus`** - Async pub-sub message bus
- **`channel`** - Channel trait and manager
- **`agent`** - LLM-powered message processing
- **`gateway`** - Unified orchestration
- **`session`** - Conversation persistence
- **`config`** - Configuration with validation
- **`error`** - Unified error handling

## Usage Examples

### Programmatic Usage

```rust
use machi_bot::prelude::*;
use machi::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Create model
    let model = AnthropicClient::from_env()
        .completion_model("claude-sonnet-4-20250514");

    // Build and run gateway
    let gateway = GatewayBuilder::new()
        .model(model)
        .load_config().await?
        .build();

    gateway.run().await
}
```

### Config Validation

```rust
use machi_bot::config::BotConfig;

let config = BotConfig::default().with_env();

for issue in config.validate() {
    println!("{}", issue);
}

if config.is_valid() {
    println!("Config is valid!");
}
```

## Features Flags

- `telegram` - Enable Telegram bot support (requires `teloxide`)

## License

Licensed under Apache 2.0 or MIT at your option.
