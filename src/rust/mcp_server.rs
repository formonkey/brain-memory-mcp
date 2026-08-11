use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router, ServerHandler,
};
use serde::Deserialize;
use serde_json::to_string_pretty;
use std::env;
use std::path::PathBuf;

use super::embedder::FastEmbedder;
use super::store::VectorStore;

#[derive(Clone)]
pub struct BrainMemoryServer {
    db_path: PathBuf,
    tool_router: ToolRouter<Self>,
}

impl BrainMemoryServer {
    pub fn new(db_path: PathBuf) -> Self {
        Self {
            db_path,
            tool_router: Self::tool_router(),
        }
    }

    fn open_store(&self) -> anyhow::Result<VectorStore> {
        Ok(VectorStore::open(
            &self.db_path,
            Box::new(FastEmbedder::new()?),
        )?)
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct IndexParams {
    folder: Option<String>,
    #[serde(default = "default_pattern")]
    pattern: String,
    #[serde(default = "default_chunk_size")]
    chunk_size: usize,
    #[serde(default = "default_overlap")]
    overlap: usize,
    #[serde(default)]
    force: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ResetParams {
    folder: Option<String>,
    #[serde(default = "default_pattern")]
    pattern: String,
    #[serde(default = "default_chunk_size")]
    chunk_size: usize,
    #[serde(default = "default_overlap")]
    overlap: usize,
    #[serde(default)]
    confirm: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchParams {
    query: String,
    #[serde(default = "default_top_k")]
    top_k: usize,
    root: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ChunkParams {
    chunk_id: i64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ClearParams {
    #[serde(default)]
    confirm: bool,
}

#[tool_router]
impl BrainMemoryServer {
    #[tool(description = "Index Markdown files from a folder into the local vector database.")]
    fn index_markdown_folder(&self, Parameters(params): Parameters<IndexParams>) -> String {
        json_result(|| {
            let folder = target_folder(params.folder)?;
            let mut store = self.open_store()?;
            let summary = store.index_folder(
                &PathBuf::from(folder),
                &params.pattern,
                params.chunk_size,
                params.overlap,
                params.force,
            )?;
            Ok(serde_json::json!(summary))
        })
    }

    #[tool(description = "Delete and rebuild the vector index for a Markdown folder.")]
    fn reset_index(&self, Parameters(params): Parameters<ResetParams>) -> String {
        json_result(|| {
            if !params.confirm {
                return Ok(serde_json::json!({
                    "reset": false,
                    "message": "Pass confirm=true to delete and rebuild the index for this folder."
                }));
            }
            let folder = target_folder(params.folder)?;
            let mut store = self.open_store()?;
            let summary = store.reset_folder(
                &PathBuf::from(folder),
                &params.pattern,
                params.chunk_size,
                params.overlap,
            )?;
            Ok(serde_json::json!({ "reset": true, "summary": summary }))
        })
    }

    #[tool(description = "Search indexed Markdown memory for the chunks most relevant to a query.")]
    fn search_memory(&self, Parameters(params): Parameters<SearchParams>) -> String {
        json_result(|| {
            let mut store = self.open_store()?;
            let root = params.root.as_deref().map(PathBuf::from);
            let results = store.search(&params.query, params.top_k, root.as_deref())?;
            Ok(serde_json::json!(results))
        })
    }

    #[tool(description = "Read a full indexed chunk by id.")]
    fn get_chunk(&self, Parameters(params): Parameters<ChunkParams>) -> String {
        json_result(|| {
            let store = self.open_store()?;
            let chunk = store
                .get_chunk(params.chunk_id)?
                .ok_or_else(|| anyhow::anyhow!("Unknown chunk_id: {}", params.chunk_id))?;
            Ok(chunk)
        })
    }

    #[tool(description = "Return database and indexing statistics.")]
    fn memory_stats(&self) -> String {
        json_result(|| {
            let store = self.open_store()?;
            store.stats()
        })
    }

    #[tool(description = "Clear the local memory index. Pass confirm=true to actually delete it.")]
    fn clear_memory(&self, Parameters(params): Parameters<ClearParams>) -> String {
        json_result(|| {
            if !params.confirm {
                return Ok(serde_json::json!({
                    "cleared": false,
                    "message": "Pass confirm=true to clear the memory index."
                }));
            }
            let store = self.open_store()?;
            store.clear()?;
            Ok(serde_json::json!({ "cleared": true }))
        })
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for BrainMemoryServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Local vector memory for Markdown documentation. Use search_memory before answering questions about indexed project notes, architecture docs, decisions, runbooks, or codebase knowledge."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

fn json_result<T>(f: impl FnOnce() -> anyhow::Result<T>) -> String
where
    T: serde::Serialize,
{
    match f() {
        Ok(value) => to_string_pretty(&value).unwrap_or_else(|_| "{\"ok\":true}".to_string()),
        Err(error) => to_string_pretty(&serde_json::json!({
            "error": error.to_string()
        }))
        .unwrap_or_else(|_| "{\"error\":\"unknown\"}".to_string()),
    }
}

fn target_folder(folder: Option<String>) -> anyhow::Result<String> {
    folder
        .or_else(|| env::var("BRAIN_MEMORY_DOCS").ok())
        .ok_or_else(|| anyhow::anyhow!("Pass folder or set BRAIN_MEMORY_DOCS"))
}

fn default_pattern() -> String {
    "**/*.md".to_string()
}

fn default_chunk_size() -> usize {
    1200
}

fn default_overlap() -> usize {
    180
}

fn default_top_k() -> usize {
    8
}
