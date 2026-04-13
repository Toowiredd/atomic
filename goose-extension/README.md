# Atomic for Goose

A [Model Context Protocol (MCP)](https://modelcontextprotocol.io/) extension that connects [Goose](https://github.com/block/goose) to your Atomic knowledge base.

## Features

- 🔍 **Semantic Search** - Search your atomic notes using natural language
- 📖 **Read Atoms** - Retrieve full content of specific atoms  
- 🌐 **Wiki Integration** - Access synthesized wiki articles
- 💬 **Chat with Context** - Include your knowledge in conversations with Goose

## Prerequisites

1. **Goose installed** - [Installation guide](https://github.com/block/goose#installation)
2. **Atomic Server running** - Start your Atomic instance:
   ```bash
   # From your atomic directory
   cargo run --release --bin atomic-server
   # Or with Docker
   docker-compose up
   ```
3. **API Token** - Get your token from Atomic Settings → API Tokens

## Quick Install

```bash
cd path/to/atomic
cargo build --release --package atomic-mcp-bridge
./goose-extension/install.sh
```

This builds the bridge and installs it to `~/.config/goose/extensions/`

## Manual Configuration

Add to your Goose config (`~/.config/goose/extensions.toml`):

```toml
[extensions.atomic]
name = "Atomic Knowledge Base"
description = "Search and chat with your personal Atomic knowledge base"
cmd = "/path/to/atomic/target/release/atomic-mcp-bridge"
type = "stdio"

[extensions.atomic.env]
ATOMIC_HOST = "127.0.0.1"
ATOMIC_PORT = "44380"
ATOMIC_TOKEN = "your-api-token-here"
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ATOMIC_HOST` | `127.0.0.1` | Atomic server hostname |
| `ATOMIC_PORT` | `44380` | Atomic server port |
| `ATOMIC_TOKEN` | *(required)* | Your Atomic API token |

## Usage in Goose

Once configured, restart Goose and you can ask:

- *"Search my Atomic notes for machine learning papers"*
- *"Read the atom with ID abc123"*
- *"What do my notes say about semantic search?"*
- *"Summarize my wiki article on distributed systems"*
- *"Using my Atomic knowledge, explain the concept of embeddings"*

## Available Tools

| Tool | Description | Parameters |
|------|-------------|------------|
| `semantic_search` | Hybrid keyword + semantic search | `query: string`, `limit?: number` |
| `read_atom` | Read full atom content | `atom_id: string`, `offset?: number`, `limit?: number` |
| `list_atoms` | Browse atoms with pagination | `limit?: number`, `offset?: number` |
| `search_wiki` | Find wiki articles | `query: string` |
| `chat_with_context` | Chat with your knowledge | `message: string`, `context_atoms?: string[]` |

## How It Works

1. **Atomic MCP Server** (`atomic-server/src/mcp/`) - Already built into Atomic
2. **MCP Bridge** (`crates/mcp-bridge/`) - stdio-to-HTTP translator for Goose
3. **Goose** - Uses the MCP protocol to communicate with Atomic

The bridge converts between Goose's stdio-based MCP protocol and Atomic's HTTP-based MCP endpoint.

## Troubleshooting

### "Connection refused" error
- Make sure Atomic server is running
- Check `ATOMIC_HOST` and `ATOMIC_PORT` match your server config

### "Authentication failed" error
- Verify your `ATOMIC_TOKEN` is correct
- Generate a new token in Atomic Settings if needed

### "No results returned"
- Ensure you have atoms in your Atomic database
- Check that embeddings have been generated (go to Settings → Re-embed All)

### MCP initialization error (like your original error)
Usually means:
- Bridge binary not found in PATH
- Atomic server not running
- Network/connectivity issue (VPN like WARP can block)
- Wrong server URL configured

## Building from Source

```bash
# Clone your fork
git clone https://github.com/Toowiredd/atomic.git
cd atomic

# Build MCP bridge
cargo build --release --package atomic-mcp-bridge

# Output: target/release/atomic-mcp-bridge
```

## Architecture

```
Goose → atomic-mcp-bridge → Atomic MCP Server → atomic-core
(stdio)    (HTTP bridge)       (RMCP protocol)    (database)
```

## License

Same as Atomic - see the main project license.
