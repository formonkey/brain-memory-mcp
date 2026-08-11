use anyhow::{anyhow, bail, Context, Result};
use bytemuck::cast_slice;
use rusqlite::ffi::sqlite3_auto_extension;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sqlite_vec::sqlite3_vec_init;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;
use walkdir::WalkDir;

use super::chunking::{chunk_markdown, TextChunk};
use super::embedder::{Embedder, EmbeddingInput};

const SCHEMA_VERSION: i64 = 1;
static REGISTER_SQLITE_VEC: Once = Once::new();

#[derive(Debug, Clone, Serialize)]
pub struct IndexSummary {
    pub root: String,
    pub files_seen: usize,
    pub files_indexed: usize,
    pub files_skipped: usize,
    pub chunks_indexed: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub chunk_id: i64,
    pub score: f64,
    pub path: String,
    pub title: Option<String>,
    pub chunk_index: i64,
    pub text: String,
}

pub struct VectorStore {
    db_path: PathBuf,
    conn: Connection,
    embedder: Box<dyn Embedder>,
}

impl VectorStore {
    pub fn open(db_path: impl Into<PathBuf>, embedder: Box<dyn Embedder>) -> Result<Self> {
        register_sqlite_vec();
        let db_path = db_path.into();
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        let conn =
            Connection::open(&db_path).with_context(|| format!("opening {}", db_path.display()))?;
        let mut store = Self {
            db_path,
            conn,
            embedder,
        };
        store.setup()?;
        Ok(store)
    }

    fn setup(&mut self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            CREATE TABLE IF NOT EXISTS metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS documents (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                root TEXT NOT NULL,
                mtime_ns INTEGER NOT NULL,
                size_bytes INTEGER NOT NULL,
                title TEXT
            );
            CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                chunk_index INTEGER NOT NULL,
                start_char INTEGER NOT NULL,
                end_char INTEGER NOT NULL,
                text TEXT NOT NULL,
                UNIQUE(document_id, chunk_index)
            );
            CREATE INDEX IF NOT EXISTS idx_documents_root ON documents(root);
            CREATE INDEX IF NOT EXISTS idx_chunks_document_id ON chunks(document_id);
            "#,
        )?;
        self.conn.execute(
            "INSERT OR REPLACE INTO metadata(key, value) VALUES (?, ?)",
            params!["schema_version", SCHEMA_VERSION.to_string()],
        )?;
        self.conn.execute(
            "INSERT OR REPLACE INTO metadata(key, value) VALUES (?, ?)",
            params!["embedding_model", self.embedder.name()],
        )?;
        let vec_version: String = self
            .conn
            .query_row("SELECT vec_version()", [], |row| row.get(0))?;
        self.conn.execute(
            "INSERT OR REPLACE INTO metadata(key, value) VALUES (?, ?)",
            params!["sqlite_vec_version", vec_version],
        )?;
        Ok(())
    }

    pub fn index_folder(
        &mut self,
        root: &Path,
        pattern: &str,
        chunk_size: usize,
        overlap: usize,
        force: bool,
    ) -> Result<IndexSummary> {
        let root = canonical_dir(root)?;
        let files = markdown_files(&root, pattern)?;
        let mut files_indexed = 0usize;
        let mut files_skipped = 0usize;
        let mut chunks_indexed = 0usize;

        for path in &files {
            let metadata = fs::metadata(path)?;
            let mtime_ns = metadata
                .modified()?
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos() as i64;
            let size_bytes = metadata.len() as i64;
            let resolved = path.canonicalize()?.to_string_lossy().to_string();

            if !force && self.is_current(&resolved, mtime_ns, size_bytes)? {
                files_skipped += 1;
                continue;
            }

            let text =
                fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
            let chunks = chunk_markdown(&text, chunk_size, overlap)?;
            let title = extract_title(&text, path);
            self.replace_document(
                &resolved,
                &root.to_string_lossy(),
                mtime_ns,
                size_bytes,
                title.as_deref(),
                &chunks,
            )?;
            files_indexed += 1;
            chunks_indexed += chunks.len();
        }

        Ok(IndexSummary {
            root: root.to_string_lossy().to_string(),
            files_seen: files.len(),
            files_indexed,
            files_skipped,
            chunks_indexed,
        })
    }

    pub fn reset_folder(
        &mut self,
        root: &Path,
        pattern: &str,
        chunk_size: usize,
        overlap: usize,
    ) -> Result<IndexSummary> {
        let root = canonical_dir(root)?;
        self.delete_vectors_for_root(&root.to_string_lossy())?;
        self.conn.execute(
            "DELETE FROM chunks WHERE document_id IN (SELECT id FROM documents WHERE root = ?)",
            params![root.to_string_lossy()],
        )?;
        self.conn.execute(
            "DELETE FROM documents WHERE root = ?",
            params![root.to_string_lossy()],
        )?;
        self.index_folder(&root, pattern, chunk_size, overlap, true)
    }

    pub fn search(
        &mut self,
        query: &str,
        top_k: usize,
        root: Option<&Path>,
    ) -> Result<Vec<SearchResult>> {
        if query.trim().is_empty() {
            bail!("query must not be empty");
        }
        if !self.has_vec_table()? {
            return Ok(Vec::new());
        }
        let top_k = top_k.clamp(1, 50);
        let query_vector = self
            .embedder
            .embed(&[query.to_string()], EmbeddingInput::Query)?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("embedder returned no query vector"))?;
        let query_blob = vector_blob(&query_vector);

        if let Some(root) = root {
            let root = canonical_dir(root)?;
            let mut stmt = self.conn.prepare(
                r#"
                WITH knn_matches AS (
                    SELECT rowid AS chunk_id, distance
                    FROM chunk_vectors
                    WHERE embedding MATCH ?
                      AND k = ?
                      AND root = ?
                )
                SELECT c.id, c.chunk_index, c.text, d.path, d.title, knn_matches.distance
                FROM knn_matches
                JOIN chunks c ON c.id = knn_matches.chunk_id
                JOIN documents d ON d.id = c.document_id
                ORDER BY knn_matches.distance
                "#,
            )?;
            let results = collect_search(stmt.query(params![
                query_blob,
                top_k as i64,
                root.to_string_lossy()
            ])?)?;
            Ok(results)
        } else {
            let mut stmt = self.conn.prepare(
                r#"
                WITH knn_matches AS (
                    SELECT rowid AS chunk_id, distance
                    FROM chunk_vectors
                    WHERE embedding MATCH ?
                      AND k = ?
                )
                SELECT c.id, c.chunk_index, c.text, d.path, d.title, knn_matches.distance
                FROM knn_matches
                JOIN chunks c ON c.id = knn_matches.chunk_id
                JOIN documents d ON d.id = c.document_id
                ORDER BY knn_matches.distance
                "#,
            )?;
            let results = collect_search(stmt.query(params![query_blob, top_k as i64])?)?;
            Ok(results)
        }
    }

    pub fn get_chunk(&self, chunk_id: i64) -> Result<Option<serde_json::Value>> {
        self.conn
            .query_row(
                r#"
                SELECT c.id, c.chunk_index, c.start_char, c.end_char, c.text, d.path, d.root, d.title
                FROM chunks c
                JOIN documents d ON d.id = c.document_id
                WHERE c.id = ?
                "#,
                params![chunk_id],
                |row| {
                    Ok(serde_json::json!({
                        "id": row.get::<_, i64>(0)?,
                        "chunk_index": row.get::<_, i64>(1)?,
                        "start_char": row.get::<_, i64>(2)?,
                        "end_char": row.get::<_, i64>(3)?,
                        "text": row.get::<_, String>(4)?,
                        "path": row.get::<_, String>(5)?,
                        "root": row.get::<_, String>(6)?,
                        "title": row.get::<_, Option<String>>(7)?,
                    }))
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn stats(&self) -> Result<serde_json::Value> {
        let documents: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))?;
        let chunks: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))?;
        let mut roots_stmt = self.conn.prepare(
            "SELECT root, COUNT(*) AS documents FROM documents GROUP BY root ORDER BY root",
        )?;
        let roots = roots_stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "root": row.get::<_, String>(0)?,
                    "documents": row.get::<_, i64>(1)?,
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(serde_json::json!({
            "db_path": self.db_path,
            "embedding_model": self.metadata_value("embedding_model")?,
            "sqlite_vec_version": self.metadata_value("sqlite_vec_version")?,
            "vector_dimensions": self.metadata_value("vector_dimensions")?,
            "documents": documents,
            "chunks": chunks,
            "roots": roots,
        }))
    }

    pub fn clear(&self) -> Result<()> {
        if self.has_vec_table()? {
            self.conn.execute("DELETE FROM chunk_vectors", [])?;
        }
        self.conn.execute("DELETE FROM chunks", [])?;
        self.conn.execute("DELETE FROM documents", [])?;
        Ok(())
    }

    fn is_current(&self, path: &str, mtime_ns: i64, size_bytes: i64) -> Result<bool> {
        let current = self
            .conn
            .query_row(
                "SELECT 1 FROM documents WHERE path = ? AND mtime_ns = ? AND size_bytes = ?",
                params![path, mtime_ns, size_bytes],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        Ok(current)
    }

    fn replace_document(
        &mut self,
        path: &str,
        root: &str,
        mtime_ns: i64,
        size_bytes: i64,
        title: Option<&str>,
        chunks: &[TextChunk],
    ) -> Result<()> {
        let existing: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM documents WHERE path = ?",
                params![path],
                |row| row.get(0),
            )
            .optional()?;

        let document_id = if let Some(document_id) = existing {
            self.delete_vectors_for_document(document_id)?;
            self.conn.execute(
                "DELETE FROM chunks WHERE document_id = ?",
                params![document_id],
            )?;
            self.conn.execute(
                "UPDATE documents SET root = ?, mtime_ns = ?, size_bytes = ?, title = ? WHERE id = ?",
                params![root, mtime_ns, size_bytes, title, document_id],
            )?;
            document_id
        } else {
            self.conn.execute(
                "INSERT INTO documents(path, root, mtime_ns, size_bytes, title) VALUES (?, ?, ?, ?, ?)",
                params![path, root, mtime_ns, size_bytes, title],
            )?;
            self.conn.last_insert_rowid()
        };

        for batch in chunks.chunks(64) {
            let texts: Vec<String> = batch.iter().map(|chunk| chunk.text.clone()).collect();
            let vectors = self.embedder.embed(&texts, EmbeddingInput::Passage)?;
            if let Some(vector) = vectors.first() {
                self.ensure_vec_table(vector.len())?;
            }
            for (chunk, vector) in batch.iter().zip(vectors.iter()) {
                self.conn.execute(
                    "INSERT INTO chunks(document_id, chunk_index, start_char, end_char, text) VALUES (?, ?, ?, ?, ?)",
                    params![document_id, chunk.index as i64, chunk.start as i64, chunk.end as i64, chunk.text],
                )?;
                let chunk_id = self.conn.last_insert_rowid();
                self.conn.execute(
                    "INSERT INTO chunk_vectors(rowid, embedding, root) VALUES (?, ?, ?)",
                    params![chunk_id, vector_blob(vector), root],
                )?;
            }
        }

        Ok(())
    }

    fn ensure_vec_table(&self, dimensions: usize) -> Result<()> {
        if self.has_vec_table()? {
            if let Some(existing) = self.metadata_value("vector_dimensions")? {
                if existing.parse::<usize>()? != dimensions {
                    bail!(
                        "Existing vector table has {existing} dimensions, but embedder produced {dimensions}. Clear or reset the index first."
                    );
                }
            }
            return Ok(());
        }

        self.conn.execute(
            &format!(
                "CREATE VIRTUAL TABLE chunk_vectors USING vec0(embedding float[{dimensions}] distance_metric=cosine, root TEXT)"
            ),
            [],
        )?;
        self.conn.execute(
            "INSERT OR REPLACE INTO metadata(key, value) VALUES (?, ?)",
            params!["vector_dimensions", dimensions.to_string()],
        )?;
        Ok(())
    }

    fn has_vec_table(&self) -> Result<bool> {
        Ok(self
            .conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'chunk_vectors'",
                [],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    fn delete_vectors_for_document(&self, document_id: i64) -> Result<()> {
        if !self.has_vec_table()? {
            return Ok(());
        }
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM chunks WHERE document_id = ?")?;
        let ids = stmt
            .query_map(params![document_id], |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        for id in ids {
            self.conn
                .execute("DELETE FROM chunk_vectors WHERE rowid = ?", params![id])?;
        }
        Ok(())
    }

    fn delete_vectors_for_root(&self, root: &str) -> Result<()> {
        if !self.has_vec_table()? {
            return Ok(());
        }
        let mut stmt = self.conn.prepare(
            "SELECT c.id FROM chunks c JOIN documents d ON d.id = c.document_id WHERE d.root = ?",
        )?;
        let ids = stmt
            .query_map(params![root], |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        for id in ids {
            self.conn
                .execute("DELETE FROM chunk_vectors WHERE rowid = ?", params![id])?;
        }
        Ok(())
    }

    fn metadata_value(&self, key: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT value FROM metadata WHERE key = ?",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }
}

fn register_sqlite_vec() {
    REGISTER_SQLITE_VEC.call_once(|| unsafe {
        sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ())));
    });
}

fn collect_search(mut rows: rusqlite::Rows<'_>) -> Result<Vec<SearchResult>> {
    let mut results = Vec::new();
    while let Some(row) = rows.next()? {
        let distance: f64 = row.get(5)?;
        results.push(SearchResult {
            chunk_id: row.get(0)?,
            chunk_index: row.get(1)?,
            text: row.get(2)?,
            path: row.get(3)?,
            title: row.get(4)?,
            score: (1.0 - distance).round_to(6),
        });
    }
    Ok(results)
}

trait RoundTo {
    fn round_to(self, decimals: i32) -> Self;
}

impl RoundTo for f64 {
    fn round_to(self, decimals: i32) -> Self {
        let factor = 10f64.powi(decimals);
        (self * factor).round() / factor
    }
}

fn vector_blob(vector: &[f32]) -> Vec<u8> {
    cast_slice(vector).to_vec()
}

fn canonical_dir(path: &Path) -> Result<PathBuf> {
    let path = path
        .canonicalize()
        .with_context(|| format!("resolving {}", path.display()))?;
    if !path.is_dir() {
        bail!("Folder does not exist: {}", path.display());
    }
    Ok(path)
}

fn markdown_files(root: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
    if pattern != "**/*.md" {
        bail!("Rust implementation currently supports pattern \"**/*.md\"");
    }
    let mut files = WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("md"))
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn extract_title(text: &str, path: &Path) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|line| line.starts_with('#'))
        .map(|line| line.trim_start_matches('#').trim().to_string())
        .filter(|title| !title.is_empty())
        .or_else(|| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rust::embedder::HashEmbedder;
    use tempfile::tempdir;

    #[test]
    fn indexes_and_searches_markdown() {
        let tmp = tempdir().unwrap();
        let docs = tmp.path().join("docs");
        fs::create_dir(&docs).unwrap();
        fs::write(
            docs.join("auth.md"),
            "# Auth\n\nTokens, sessions and login flows.",
        )
        .unwrap();
        fs::write(
            docs.join("billing.md"),
            "# Billing\n\nInvoices and payment retries.",
        )
        .unwrap();

        let mut store = VectorStore::open(
            tmp.path().join("brain.sqlite3"),
            Box::new(HashEmbedder::new(64)),
        )
        .unwrap();
        let summary = store
            .index_folder(&docs, "**/*.md", 220, 20, false)
            .unwrap();
        let results = store.search("login session token", 1, None).unwrap();

        assert_eq!(summary.files_seen, 2);
        assert_eq!(summary.files_indexed, 2);
        assert_eq!(summary.chunks_indexed, 2);
        assert!(results[0].path.ends_with("auth.md"));
    }
}
