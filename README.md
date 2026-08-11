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

Recommended install from GitHub:

```bash
cargo install --git ssh://git@github.com/formonkey/brain-memory-mcp.git --bin brain-memory-mcp-rs
```

This installs the executable into Cargo's bin directory:

- macOS/Linux: `~/.cargo/bin/brain-memory-mcp-rs`
- Windows: `%USERPROFILE%\.cargo\bin\brain-memory-mcp-rs.exe`

Make sure Cargo's bin directory is in your `PATH`.

For local development, clone and build manually:

```bash
git clone git@github.com:formonkey/brain-memory-mcp.git
cd brain-memory-mcp
cargo build --release --bin brain-memory-mcp-rs
```

The first indexing run may download the configured embedding model.

## Configuration

Environment variables:

- `BRAIN_MEMORY_DB`: SQLite database path. Defaults to `.brain-memory/brain_memory.sqlite3`.
- `BRAIN_MEMORY_DOCS`: default folder for `index_markdown_folder` and `reset_index`.
- `BRAIN_MEMORY_CONTEXT`: default logical memory context. Defaults to `default`.
- `BRAIN_MEMORY_MODEL`: embedding model. Rust defaults to `intfloat/multilingual-e5-small`.

Recommended database locations:

- `~/.codex/brain-memory/brain.sqlite3` for a Codex-wide memory database.
- `.brain-memory/brain_memory.sqlite3` for a project-local memory database.

Example:

```bash
export BRAIN_MEMORY_DOCS="/Users/me/dev-notes"
export BRAIN_MEMORY_DB="$HOME/.codex/brain-memory/brain.sqlite3"
export BRAIN_MEMORY_CONTEXT="my-project"
```

## Contexts

A single SQLite database can contain multiple isolated memory contexts.

Use contexts to separate projects, clients, teams, or knowledge domains while keeping one database under `~/.codex/brain-memory`.

Examples:

- `brain-memory-mcp`
- `jira-issue-context-mcp`
- `client-acme`
- `personal-notes`

Every indexed document belongs to one `context` and one physical `root`.

- `context`: logical namespace, usually a project or knowledge domain.
- `root`: actual folder path that was indexed.

Searches are filtered by context first, so results from different projects do not mix unless you explicitly ask for global stats.

## CLI Usage

Index a folder:

```bash
brain-memory-mcp-rs --db "$HOME/.codex/brain-memory/brain.sqlite3" --context my-project index /path/to/docs
```

Search:

```bash
brain-memory-mcp-rs --db "$HOME/.codex/brain-memory/brain.sqlite3" --context my-project search "how is auth configured?"
```

Rebuild a folder index:

```bash
brain-memory-mcp-rs --db "$HOME/.codex/brain-memory/brain.sqlite3" --context my-project reset /path/to/docs
```

Show stats:

```bash
brain-memory-mcp-rs --db "$HOME/.codex/brain-memory/brain.sqlite3" --context my-project stats
```

Show stats for all contexts:

```bash
brain-memory-mcp-rs --db "$HOME/.codex/brain-memory/brain.sqlite3" stats --all-contexts
```

Run as an MCP server over stdio:

```bash
brain-memory-mcp-rs --db "$HOME/.codex/brain-memory/brain.sqlite3" --context my-project serve
```

Run diagnostics:

```bash
brain-memory-mcp-rs --db "$HOME/.codex/brain-memory/brain.sqlite3" --context my-project doctor
```

`doctor` validates that the binary can:

- open and write to SQLite
- load `sqlite-vec`
- load the embedding model
- index a temporary Markdown document
- run semantic search
- clean up its temporary diagnostic context

During development you can also run:

```bash
cargo run --bin brain-memory-mcp-rs -- --db .brain-memory/brain.sqlite3 --context my-project serve
```

## Windows Corporate Environments

This project does not require downloading a prebuilt `.exe` release. The recommended Windows flow is source-based:

```powershell
choco install rustup.install -y
rustup default stable-msvc
cargo install --git ssh://git@github.com/formonkey/brain-memory-mcp.git --bin brain-memory-mcp-rs
```

Then validate the installation:

```powershell
C:\Users\me\.cargo\bin\brain-memory-mcp-rs.exe --db C:\Users\me\.codex\brain-memory\brain.sqlite3 --context my-project doctor
```

For Codex, prefer the explicit Cargo bin path if the desktop app does not inherit your shell `PATH`:

```toml
[mcp_servers.brain-memory]
command = "C:\\Users\\me\\.cargo\\bin\\brain-memory-mcp-rs.exe"
args = [
  "--db",
  "C:\\Users\\me\\.codex\\brain-memory\\brain.sqlite3",
  "serve",
]
startup_timeout_sec = 120

[mcp_servers.brain-memory.env]
BRAIN_MEMORY_DOCS = "C:\\Users\\me\\dev-notes"
BRAIN_MEMORY_CONTEXT = "my-project"
BRAIN_MEMORY_MODEL = "intfloat/multilingual-e5-small"
```

Common corporate blockers:

- SSH access to GitHub may be blocked. Use the HTTPS Git URL if needed:
  `cargo install --git https://github.com/formonkey/brain-memory-mcp.git --bin brain-memory-mcp-rs`.
- The first model download may be blocked by proxy or firewall rules.
- Antivirus tools may slow down the first build because `fastembed-rs` and ONNX dependencies compile native code.
- If Codex cannot start the MCP, use the full path to the binary instead of relying on `PATH`.

## MCP Tools

### `index_markdown_folder`

Indexes a folder of `*.md` files.

Parameters:

- `folder`: folder to index. Optional when `BRAIN_MEMORY_DOCS` is set.
- `context`: logical namespace. Optional when `BRAIN_MEMORY_CONTEXT` is set.
- `pattern`: file pattern. Defaults to `**/*.md`.
- `chunk_size`: chunk size. Defaults to `1200`.
- `overlap`: overlap between chunks. Defaults to `180`.
- `force`: reindex even when a file appears unchanged.

Expected use: after adding or editing documentation.

### `reset_index`

Deletes and rebuilds the index for one folder.

Parameters:

- `folder`: folder to rebuild. Optional when `BRAIN_MEMORY_DOCS` is set.
- `context`: logical namespace. Optional when `BRAIN_MEMORY_CONTEXT` is set.
- `pattern`: file pattern. Defaults to `**/*.md`.
- `chunk_size`: chunk size. Defaults to `1200`.
- `overlap`: overlap between chunks. Defaults to `180`.
- `confirm`: must be `true`.

Expected use: after deleting files, changing a lot of documentation, or requesting a clean rebuild.

### `search_memory`

Searches relevant chunks by semantic similarity.

Parameters:

- `query`: question or search text.
- `context`: logical namespace. Optional when `BRAIN_MEMORY_CONTEXT` is set.
- `top_k`: number of results. Defaults to `8`.
- `root`: limit the search to one indexed folder.

This is the main tool the agent should use.

### `get_chunk`

Returns an exact chunk by `chunk_id`.

Expected use: expand a specific result returned by `search_memory`.

### `memory_stats`

Returns database statistics:

Parameters:

- `context`: logical namespace to inspect. Optional when `BRAIN_MEMORY_CONTEXT` is set.
- `all_contexts`: show all contexts when `true`.

Returned fields include:

- SQLite path
- selected context
- embedding model
- `sqlite-vec` version
- vector dimensions
- known contexts
- document count
- chunk count
- indexed roots

### `clear_memory`

Deletes memory for one context by default.

Parameters:

- `context`: logical namespace to clear. Optional when `BRAIN_MEMORY_CONTEXT` is set.
- `all_contexts`: clear every context when `true`.
- `confirm`: must be `true`.

Expected use: context reset by default. Use `all_contexts=true` only for a full database reset.

## MCP Client Config

Example for an MCP client:

```json
{
  "mcpServers": {
    "brain-memory": {
      "command": "brain-memory-mcp-rs",
      "args": [
        "--db",
        "/Users/me/.codex/brain-memory/brain.sqlite3",
        "serve"
      ],
      "env": {
        "BRAIN_MEMORY_DOCS": "/path/to/markdown",
        "BRAIN_MEMORY_CONTEXT": "my-project",
        "BRAIN_MEMORY_MODEL": "intfloat/multilingual-e5-small"
      }
    }
  }
}
```

## Configure in Codex

In Codex, local MCP servers are declared in `~/.codex/config.toml`.

First install the MCP from GitHub:

```bash
cargo install --git ssh://git@github.com/formonkey/brain-memory-mcp.git --bin brain-memory-mcp-rs
```

Then add an entry like this, replacing `/Users/me` and `/path/to/markdown` with your real paths:

```toml
[mcp_servers.brain-memory]
command = "brain-memory-mcp-rs"
args = [
  "--db",
  "/Users/me/.codex/brain-memory/brain.sqlite3",
  "serve",
]
startup_timeout_sec = 120

[mcp_servers.brain-memory.env]
BRAIN_MEMORY_DOCS = "/path/to/markdown"
BRAIN_MEMORY_CONTEXT = "my-project"
BRAIN_MEMORY_MODEL = "intfloat/multilingual-e5-small"
```

If `brain-memory-mcp-rs` is not in Codex's `PATH`, use the full Cargo bin path instead:

```toml
[mcp_servers.brain-memory]
command = "/Users/me/.cargo/bin/brain-memory-mcp-rs"
args = [
  "--db",
  "/Users/me/.codex/brain-memory/brain.sqlite3",
  "serve",
]
startup_timeout_sec = 120

[mcp_servers.brain-memory.env]
BRAIN_MEMORY_DOCS = "/path/to/markdown"
BRAIN_MEMORY_CONTEXT = "my-project"
BRAIN_MEMORY_MODEL = "intfloat/multilingual-e5-small"
```

On Windows:

```toml
[mcp_servers.brain-memory]
command = "C:\\Users\\me\\.cargo\\bin\\brain-memory-mcp-rs.exe"
args = [
  "--db",
  "C:\\Users\\me\\.codex\\brain-memory\\brain.sqlite3",
  "serve",
]
startup_timeout_sec = 120

[mcp_servers.brain-memory.env]
BRAIN_MEMORY_DOCS = "C:\\Users\\me\\dev-notes"
BRAIN_MEMORY_CONTEXT = "my-project"
BRAIN_MEMORY_MODEL = "intfloat/multilingual-e5-small"
```

After editing `~/.codex/config.toml`, restart Codex so it loads the new MCP server.

Before connecting it to Codex, verify that the binary works:

```bash
brain-memory-mcp-rs --db "$HOME/.codex/brain-memory/brain.sqlite3" --context my-project stats
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

Default memory context: `my-project`.

Before answering questions about architecture, technical decisions, runbooks, known issues, configuration, project flows, or historical knowledge:

1. Use `search_memory` with a short semantic query and the project context.
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

Use the configured `BRAIN_MEMORY_CONTEXT` by default. If the user names a project/client/domain, pass that value as `context`.

## Workflow

1. For project knowledge questions, call `search_memory` first with the relevant `context`.
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
3. Set `BRAIN_MEMORY_CONTEXT` to the project or domain name.
4. Run `index_markdown_folder` or the CLI `index` command.
5. Let the agent use `search_memory` before answering questions about that knowledge.
6. Use `reset_index` when files were deleted or when you want a clean rebuild.

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

CI runs these checks on `ubuntu-latest`, `macos-latest`, and `windows-latest`.

## References

- Rust/Cargo recommend installing Rust with `rustup`: https://doc.rust-lang.org/stable/cargo/getting-started/installation.html
- Rustup: https://rust-lang.github.io/rustup/installation/
- Homebrew `rustup`: https://formulae.brew.sh/formula/rustup
- Chocolatey `rustup.install`: https://community.chocolatey.org/packages/rustup.install
