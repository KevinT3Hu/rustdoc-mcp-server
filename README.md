# rustdoc-mcp

A [Model Context Protocol (MCP)](https://modelcontextprotocol.io/) server that provides real-time access to Rust documentation for your project dependencies.

It allows LLMs to query crate documentation, search for items, and retrieve detailed docs for structs, functions, traits, and more, all by leveraging `cargo rustdoc`'s JSON output format.

## Features

- **Dependency Discovery**: List all dependencies in your current project.
- **On-demand Documentation**: Generates documentation for crates as needed using the nightly toolchain.
- **Search**: Fuzzy search across available documentation.
- **Module Exploration**: Browse items within modules.
- **Detailed Docs**: Retrieve full Markdown documentation for any item.

## Prerequisites

- **Rust Nightly Toolchain**: This server relies on the experimental JSON output from `rustdoc`, which is currently only available in nightly.
  ```bash
  rustup toolchain install nightly
  ```

## Installation

### From Source

1.  Clone the repository:
    ```bash
    git clone https://github.com/KevinT3Hu/rustdoc-mcp.git
    cd rustdoc-mcp
    ```

2.  Build the project:
    ```bash
    cargo build --release
    ```

    The binary will be available at `target/release/rustdoc-mcp`.

## Usage

### Integrating with Claude Desktop

Add the server to your Claude Desktop configuration file (typically located at `~/Library/Application Support/Claude/claude_desktop_config.json` on macOS or `%APPDATA%\Claude\claude_desktop_config.json` on Windows).

Replace `/path/to/rustdoc-mcp` with the actual path to your built binary.

```json
{
  "mcpServers": {
    "rustdoc": {
      "command": "/path/to/rustdoc-mcp/target/release/rustdoc-mcp",
      "args": [
        "start",
        "--cwd",
        "/path/to/your/rust/project"
      ]
    }
  }
}
```

*Note: The `--cwd` argument is optional. If omitted, it defaults to the current working directory of the process, but specifying the target project path is recommended.*

### Available Tools

When the server is running, the following tools are available to the LLM:

- **`list_deps`**: Lists all dependencies available in the current project.
- **`list_crate_items`**: Lists root items of a specific crate (e.g., `std`, `tokio`).
- **`search_docs`**: Performs a fuzzy search for items matching a query.
- **`get_module`**: Returns a summary of public items within a specific module path.
- **`get_docs`**: Returns the full markdown documentation for a specific item path (e.g., `std::vec::Vec`).

## How it Works

1.  The server inspects the `Cargo.toml` of the target project to find dependencies.
2.  When documentation is requested for a crate, it runs `cargo +nightly rustdoc` to generate JSON documentation.
3.  The JSON is cached and indexed in memory for fast retrieval.
4.  Queries are processed against this index to return Markdown-formatted documentation.

## Troubleshooting

- **Logs**: The server writes logs to `/tmp/rustdoc-mcp/server.log`. Check this file if you encounter issues. To get more detailed logs, set the `RUST_LOG` environment variable before starting the server:
  ```bash
  export RUST_LOG=debug
  ```
- **Compilation Errors**: Since the server runs `cargo rustdoc`, ensure your project compiles successfully.

## License

This project is licensed under the GNU Lesser General Public License v3.0 - see the [LICENSE](LICENSE) file for details.
