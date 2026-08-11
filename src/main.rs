mod rust;

use anyhow::Result;
use clap::{Parser, Subcommand};
use rmcp::{transport::stdio, ServiceExt};
use serde_json::to_string_pretty;
use std::path::PathBuf;

use crate::rust::embedder::FastEmbedder;
use crate::rust::mcp_server::BrainMemoryServer;
use crate::rust::store::VectorStore;

#[derive(Debug, Parser)]
#[command(name = "brain-memory-mcp-rs")]
#[command(about = "Rust Brain Memory MCP")]
struct Cli {
    #[arg(
        long,
        env = "BRAIN_MEMORY_DB",
        default_value = ".brain-memory/brain_memory.sqlite3"
    )]
    db: PathBuf,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    Serve,
    Index {
        folder: PathBuf,
        #[arg(long, default_value = "**/*.md")]
        pattern: String,
        #[arg(long, default_value_t = 1200)]
        chunk_size: usize,
        #[arg(long, default_value_t = 180)]
        overlap: usize,
        #[arg(long)]
        force: bool,
    },
    Reset {
        folder: PathBuf,
        #[arg(long, default_value = "**/*.md")]
        pattern: String,
        #[arg(long, default_value_t = 1200)]
        chunk_size: usize,
        #[arg(long, default_value_t = 180)]
        overlap: usize,
    },
    Search {
        query: String,
        #[arg(long, default_value_t = 8)]
        top_k: usize,
        #[arg(long)]
        root: Option<PathBuf>,
    },
    Stats,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => {
            let service = BrainMemoryServer::new(cli.db).serve(stdio()).await?;
            service.waiting().await?;
        }
        Command::Index {
            folder,
            pattern,
            chunk_size,
            overlap,
            force,
        } => {
            let mut store = VectorStore::open(cli.db, Box::new(FastEmbedder::new()?))?;
            let summary = store.index_folder(&folder, &pattern, chunk_size, overlap, force)?;
            println!("{}", to_string_pretty(&summary)?);
        }
        Command::Reset {
            folder,
            pattern,
            chunk_size,
            overlap,
        } => {
            let mut store = VectorStore::open(cli.db, Box::new(FastEmbedder::new()?))?;
            let summary = store.reset_folder(&folder, &pattern, chunk_size, overlap)?;
            println!(
                "{}",
                to_string_pretty(&serde_json::json!({ "reset": true, "summary": summary }))?
            );
        }
        Command::Search { query, top_k, root } => {
            let mut store = VectorStore::open(cli.db, Box::new(FastEmbedder::new()?))?;
            let results = store.search(&query, top_k, root.as_deref())?;
            println!("{}", to_string_pretty(&results)?);
        }
        Command::Stats => {
            let store = VectorStore::open(cli.db, Box::new(FastEmbedder::new()?))?;
            println!("{}", to_string_pretty(&store.stats()?)?);
        }
    }

    Ok(())
}
