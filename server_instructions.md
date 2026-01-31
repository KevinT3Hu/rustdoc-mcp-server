# RustDoc MCP Server Instructions

This server provides access to Rust documentation for the current project and its dependencies. It generates documentation on the fly using `cargo rustdoc` (requiring the nightly toolchain) and allows you to explore crates, modules, and items.

## Tools

### `list_deps`
Returns a list of all dependencies available in the current project. Use this to find out which crates are available for documentation queries.

### `list_crate_items`
Lists the root items of a specific crate.
- `crate_name`: The name of the crate (e.g., "serde", "tokio", or the current project name).

### `get_docs`
Returns the full markdown documentation for a specific item path.
- `path`: The full path to the item (e.g., `tokio::net::TcpStream`).

### `search_docs`
Performs a fuzzy search across the index for items matching the query.
- `query`: The search string.
- `crate_name`: (Optional) Limit search to a specific crate.

### `get_module`
Returns a summary of all public items within a specific module path.
- `path`: The full path to the module (e.g., `tokio::process`).

## Recommended Workflow

1.  **Explore Dependencies**: Start by running `list_deps` to see what crates are available.
2.  **Locate Items**:
    *   If you know the crate but not the item, use `list_crate_items` to see the root.
    *   If you are looking for something specific, use `search_docs`.
3.  **Browse Modules**: Use `get_module` to explore the contents of a module found in the previous steps.
4.  **Read Documentation**: Once you have the path to an item (struct, enum, function, trait, etc.), use `get_docs` to read its detailed documentation, including examples and method signatures.

## Notes
- The server requires the **nightly** Rust toolchain.
- Documentation is generated on-demand, so the first request for a crate might take a moment.
- Paths must be exact for `get_docs` and `get_module`. Use `search_docs` if you are unsure of the path.
