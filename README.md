# Brain Memory MCP

Local MCP server that turns a folder full of `*.md` files into a vectorized memory that Codex, Claude Desktop, or any other MCP client can query.

The idea is simple: index documentation, notes, architecture decisions, runbooks, or project learnings, then let the agent retrieve relevant fragments with semantic search instead of relying on `grep` or manually reading thousands of files.

## Status

This implementation is Rust-only:

- `rmcp`: MCP server over stdio.
- `fastembed-rs`: local embeddings.
- `sqlite-vec`: vector search inside SQLite.
- `rusqlite`: local storage.

## Repository

```bash
git clone git@github.com:formonkey/brain-memory-mcp.git
cd brain-memory-mcp
```

## What It Does

- Walks a Markdown folder.
- Splits each file into overlapping chunks.
- Generates local embeddings for each chunk.
- Stores documents and chunks in SQLite.
- Stores and queries vectors with `sqlite-vec`.
- Exposes MCP tools for indexing, searching, expanding results, inspecting state, and clearing memory.

## Install Rust

The recommended way to work with Rust is `rustup`, which installs and manages `rustc`, `cargo`, and toolchains.

### Windows with Chocolatey

Open PowerShell as administrator:

```powershell
choco install rustup.install -y
rustup default stable-msvc
rustc --version
cargo --version
```

If Chocolatey is not installed, install it first using the official Chocolatey documentation.

### macOS with Homebrew

```bash
brew install rustup
rustup default stable
rustc --version
cargo --version
```

If `rustup`, `rustc`, or `cargo` are not in your `PATH`, add Homebrew's `rustup` bin directory:

```bash
echo 'export PATH="$(brew --prefix rustup)/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

### Linux

Official installer:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustc --version
cargo --version
```

On modern distributions that package `rustup`, you can also use the system package manager, for example:

```bash
sudo apt install rustup
rustup default stable
```

## Install the MCP Server

Clone the repository and build the Rust binary:

```bash
git clone git@github.com:formonkey/brain-memory-mcp.git
cd brain-memory-mcp
cargo build --release --bin brain-memory-mcp-rs
```

The binary is created at:

```bash
./target/release/brain-memory-mcp-rs
```

The first indexing run may download the configured embedding model.

## Configuration

Environment variables:

- `BRAIN_MEMORY_DB`: SQLite database path. Defaults to `.brain-memory/brain_memory.sqlite3`.
- `BRAIN_MEMORY_DOCS`: default folder for `index_markdown_folder` and `reset_index`.
- `BRAIN_MEMORY_MODEL`: embedding model. Rust defaults to `intfloat/multilingual-e5-small`.

Recommended database locations:

- `~/.codex/brain-memory/brain.sqlite3` for a Codex-wide memory database.
- `.brain-memory/brain_memory.sqlite3` for a project-local memory database.

Example:

```bash
export BRAIN_MEMORY_DOCS="/Users/me/dev-notes"
export BRAIN_MEMORY_DB="$HOME/.codex/brain-memory/brain.sqlite3"
```

## CLI Usage

Index a folder:

```bash
target/release/brain-memory-mcp-rs --db "$HOME/.codex/brain-memory/brain.sqlite3" index /path/to/docs
```

Search:

```bash
target/release/brain-memory-mcp-rs --db "$HOME/.codex/brain-memory/brain.sqlite3" search "how is auth configured?"
```

Rebuild a folder index:

```bash
target/release/brain-memory-mcp-rs --db "$HOME/.codex/brain-memory/brain.sqlite3" reset /path/to/docs
```

Show stats:

```bash
target/release/brain-memory-mcp-rs --db "$HOME/.codex/brain-memory/brain.sqlite3" stats
```

Run as an MCP server over stdio:

```bash
target/release/brain-memory-mcp-rs --db "$HOME/.codex/brain-memory/brain.sqlite3" serve
```

During development you can also run:

```bash
cargo run --bin brain-memory-mcp-rs -- --db .brain-memory/brain.sqlite3 serve
```

## MCP Tools

### `index_markdown_folder`

Indexes a folder of `*.md` files.

Parameters:

- `folder`: folder to index. Optional when `BRAIN_MEMORY_DOCS` is set.
- `pattern`: file pattern. Defaults to `**/*.md`.
- `chunk_size`: chunk size. Defaults to `1200`.
- `overlap`: overlap between chunks. Defaults to `180`.
- `force`: reindex even when a file appears unchanged.

Expected use: after adding or editing documentation.

### `reset_index`

Deletes and rebuilds the index for one folder.

Parameters:

- `folder`: folder to rebuild. Optional when `BRAIN_MEMORY_DOCS` is set.
- `pattern`: file pattern. Defaults to `**/*.md`.
- `chunk_size`: chunk size. Defaults to `1200`.
- `overlap`: overlap between chunks. Defaults to `180`.
- `confirm`: must be `true`.

Expected use: after deleting files, changing a lot of documentation, or requesting a clean rebuild.

### `search_memory`

Searches relevant chunks by semantic similarity.

Parameters:

- `query`: question or search text.
- `top_k`: number of results. Defaults to `8`.
- `root`: limit the search to one indexed folder.

This is the main tool the agent should use.

### `get_chunk`

Returns an exact chunk by `chunk_id`.

Expected use: expand a specific result returned by `search_memory`.

### `memory_stats`

Returns database statistics:

- SQLite path
- embedding model
- `sqlite-vec` version
- vector dimensions
- document count
- chunk count
- indexed roots

### `clear_memory`

Deletes all local memory.

Parameters:

- `confirm`: must be `true`.

Expected use: full database reset, not just one folder.

## MCP Client Config

Example for an MCP client:

```json
{
  "mcpServers": {
    "brain-memory": {
      "command": "/absolute/path/to/brain-memory-mcp/target/release/brain-memory-mcp-rs",
      "args": [
        "--db",
        "/Users/me/.codex/brain-memory/brain.sqlite3",
        "serve"
      ],
      "env": {
        "BRAIN_MEMORY_DOCS": "/path/to/markdown",
        "BRAIN_MEMORY_MODEL": "intfloat/multilingual-e5-small"
      }
    }
  }
}
```

## Configure in Codex

In Codex, local MCP servers are declared in `~/.codex/config.toml`. Add an entry like this:

```toml
[mcp_servers.brain-memory]
command = "/absolute/path/to/brain-memory-mcp/target/release/brain-memory-mcp-rs"
args = [
  "--db",
  "/Users/me/.codex/brain-memory/brain.sqlite3",
  "serve",
]
startup_timeout_sec = 120

[mcp_servers.brain-memory.env]
BRAIN_MEMORY_DOCS = "/path/to/markdown"
BRAIN_MEMORY_MODEL = "intfloat/multilingual-e5-small"
```

On Windows, use absolute Windows paths and escape backslashes when using regular TOML strings:

```toml
[mcp_servers.brain-memory]
command = "C:\\Users\\me\\dev\\brain-memory-mcp\\target\\release\\brain-memory-mcp-rs.exe"
args = [
  "--db",
  "C:\\Users\\me\\.codex\\brain-memory\\brain.sqlite3",
  "serve",
]
startup_timeout_sec = 120

[mcp_servers.brain-memory.env]
BRAIN_MEMORY_DOCS = "C:\\Users\\me\\dev-notes"
BRAIN_MEMORY_MODEL = "intfloat/multilingual-e5-small"
```

After editing `~/.codex/config.toml`, restart Codex so it loads the new MCP server.

Before connecting it to Codex, verify that the binary works:

```bash
./target/release/brain-memory-mcp-rs --db "$HOME/.codex/brain-memory/brain.sqlite3" stats
```

## Create an Agent That Uses the Memory

You can guide Codex from a project-level `AGENTS.md`. Create or edit:

```bash
touch AGENTS.md
```

Recommended example:

```markdown
# Brain Memory

This project uses the `brain-memory` MCP server to retrieve context from vectorized Markdown documentation.

Before answering questions about architecture, technical decisions, runbooks, known issues, configuration, project flows, or historical knowledge:

1. Use `search_memory` with a short semantic query.
2. If the result seems incomplete, rephrase the query and search again.
3. If a specific result needs more context, use `get_chunk` with its `chunk_id`.
4. When useful, mention the document paths used in the answer.

Maintenance:

- If the user says Markdown was added or changed, use `index_markdown_folder`.
- If the user says many Markdown files were deleted or asks for a clean rebuild, use `reset_index` with `confirm=true`.
- Use `memory_stats` to inspect index state.
- Do not use `clear_memory` unless the user explicitly asks for it.
```

This `AGENTS.md` is useful when you want every agent working in that repo to remember that it should consult memory before answering local-knowledge questions.

## Create a Codex Skill

If you want this behavior to activate across projects, create a skill in `$CODEX_HOME/skills` or, when `CODEX_HOME` is not set, in `~/.codex/skills`.

Structure:

```text
~/.codex/skills/brain-memory/
  SKILL.md
```

Recommended contents for `~/.codex/skills/brain-memory/SKILL.md`:

```markdown
---
name: brain-memory
description: Use when answering questions that may depend on local Markdown memory, architecture decisions, runbooks, project notes, historical fixes, or when the user asks to index, reset, or search documentation through the brain-memory MCP.
---

# Brain Memory

Use the `brain-memory` MCP server to retrieve relevant local Markdown context before answering questions about project knowledge.

## Workflow

1. For project knowledge questions, call `search_memory` first.
2. Prefer concise semantic queries over long prompts.
3. If results are weak, try 1-2 alternative queries.
4. Use `get_chunk` when a returned `chunk_id` needs more exact context.
5. Mention source document paths when they help the user verify the answer.

## Index Maintenance

- Use `index_markdown_folder` when documentation has been added or edited.
- Use `reset_index` with `confirm=true` when documentation was deleted or a clean rebuild is requested.
- Use `memory_stats` to inspect the index.
- Use `clear_memory` only after an explicit user request.

## Response Style

When the answer depends on retrieved memory, say that it comes from indexed Markdown context and cite the relevant file paths. If no useful result is found, say that the memory index did not contain enough context.
```

To validate that Codex sees the skill, restart Codex and try a prompt that should trigger it:

```text
Use brain-memory to search what we decided about authentication.
```

## Relationship Between Codex, Skill, and MCP

```text
User asks a project question
        |
        v
Codex reads AGENTS.md or activates the brain-memory skill
        |
        v
Codex calls tools from the brain-memory MCP
        |
        v
brain-memory searches SQLite + sqlite-vec
        |
        v
Codex answers using the relevant Markdown chunks
```

## Recommended Workflow

1. Put your notes or documentation in a Markdown folder.
2. Set `BRAIN_MEMORY_DOCS` to that folder.
3. Run `index_markdown_folder` or the CLI `index` command.
4. Let the agent use `search_memory` before answering questions about that knowledge.
5. Use `reset_index` when files were deleted or when you want a clean rebuild.

## Performance

With the default Rust model (`intfloat/multilingual-e5-small`), vectors are small and suitable for local development memory.

Approximate guidance:

- `10k-50k chunks`: should be very fast.
- `100k-300k chunks`: still reasonable on a good laptop.
- `500k+ chunks`: measure real latency and database size.
- `1M+ chunks`: consider Qdrant, LanceDB, or Chroma if you need consistently comfortable latency.

The main bottleneck is usually the first vectorization pass, not querying.

## Development

```bash
cargo fmt --check
cargo test
cargo build --release --bin brain-memory-mcp-rs
```

## References

- Rust/Cargo recommend installing Rust with `rustup`: https://doc.rust-lang.org/stable/cargo/getting-started/installation.html
- Rustup: https://rust-lang.github.io/rustup/installation/
- Homebrew `rustup`: https://formulae.brew.sh/formula/rustup
- Chocolatey `rustup.install`: https://community.chocolatey.org/packages/rustup.install
