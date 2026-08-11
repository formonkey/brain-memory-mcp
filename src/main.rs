mod rust;

use anyhow::Result;
use clap::{Parser, Subcommand};
use rmcp::{transport::stdio, ServiceExt};
use serde_json::{json, to_string_pretty};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::rust::embedder::FastEmbedder;
use crate::rust::mcp_server::BrainMemoryServer;
use crate::rust::store::{VectorStore, DEFAULT_CONTEXT};

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

    #[arg(long, env = "BRAIN_MEMORY_CONTEXT", default_value = DEFAULT_CONTEXT)]
    context: String,

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
    Stats {
        #[arg(long)]
        all_contexts: bool,
    },
    Doctor,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => {
            let service = BrainMemoryServer::new(cli.db, cli.context)
                .serve(stdio())
                .await?;
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
            let summary =
                store.index_folder(&cli.context, &folder, &pattern, chunk_size, overlap, force)?;
            println!("{}", to_string_pretty(&summary)?);
        }
        Command::Reset {
            folder,
            pattern,
            chunk_size,
            overlap,
        } => {
            let mut store = VectorStore::open(cli.db, Box::new(FastEmbedder::new()?))?;
            let summary =
                store.reset_folder(&cli.context, &folder, &pattern, chunk_size, overlap)?;
            println!(
                "{}",
                to_string_pretty(&serde_json::json!({ "reset": true, "summary": summary }))?
            );
        }
        Command::Search { query, top_k, root } => {
            let mut store = VectorStore::open(cli.db, Box::new(FastEmbedder::new()?))?;
            let results = store.search(&query, top_k, &cli.context, root.as_deref())?;
            println!("{}", to_string_pretty(&results)?);
        }
        Command::Stats { all_contexts } => {
            let store = VectorStore::open(cli.db, Box::new(FastEmbedder::new()?))?;
            let context = if all_contexts {
                None
            } else {
                Some(cli.context.as_str())
            };
            println!("{}", to_string_pretty(&store.stats(context)?)?);
        }
        Command::Doctor => {
            let report = run_doctor(&cli.db, &cli.context)?;
            println!("{}", to_string_pretty(&report)?);
        }
    }

    Ok(())
}

fn run_doctor(db_path: &Path, context: &str) -> Result<serde_json::Value> {
    let doctor_context = format!("__doctor__-{context}");
    let temp_root = std::env::temp_dir().join(format!(
        "brain-memory-doctor-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    ));
    fs::create_dir_all(&temp_root)?;
    let doctor_doc = temp_root.join("doctor.md");
    fs::write(
        &doctor_doc,
        "# Brain Memory Doctor\n\nThis document validates SQLite, sqlite-vec, embeddings, indexing, and semantic search.",
    )?;

    let mut store = VectorStore::open(db_path, Box::new(FastEmbedder::new()?))?;
    store.clear_context(&doctor_context)?;
    let index_summary =
        store.index_folder(&doctor_context, &temp_root, "**/*.md", 220, 20, true)?;
    let search_results = store.search(
        "validate embeddings semantic search",
        3,
        &doctor_context,
        None,
    )?;
    let stats = store.stats(Some(&doctor_context))?;
    store.clear_context(&doctor_context)?;
    let _ = fs::remove_dir_all(&temp_root);

    Ok(json!({
        "ok": !search_results.is_empty(),
        "db_path": db_path,
        "context_under_test": doctor_context,
        "checks": {
            "sqlite_open": true,
            "sqlite_vec_loaded": true,
            "embedding_model_loaded": true,
            "index_markdown_folder": index_summary,
            "search_memory_results": search_results.len(),
            "memory_stats": stats
        }
    }))
}
