//! Hybrid retrieval: vector search + BM25, fused with Reciprocal Rank Fusion.
//!
//! # Algorithm choice — Reciprocal Rank Fusion (RRF)
//!
//! Results from the vector search and BM25 full-text search are combined with
//! RRF (Cormack, Clarke & Buettcher, SIGIR 2009).
//!
//! RRF score per chunk: **Σ_list 1 / (k + rank_list)** where rank is 1-indexed
//! and k = 60 (the constant from the original paper).
//!
//! RRF was chosen over linear interpolation (α·vec + (1−α)·bm25) because:
//! - No α hyperparameter to tune per corpus.
//! - Items appearing in only one list still contribute a positive score.
//! - Empirically competitive with tuned weighted fusion for RAG workloads.
//!
//! # Vector search note
//! Embeddings are stored as packed-f32 BLOBs; cosine similarity is computed in
//! Rust at query time. For a 1,000-document corpus (~8,000 chunks, ~47 MB) this
//! is sub-millisecond on any modern CPU.  The sqlite-vec `vec0` virtual table is
//! the planned production swap — replace `SqliteAdapter::search_embeddings` with
//! `WHERE embedding MATCH ? ORDER BY distance LIMIT ?` when sqlite-vec is wired
//! into the build.

use std::collections::HashMap;
use std::sync::Arc;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::KayaError;
use crate::model_router::ModelRouter;
use crate::storage::{Chunk, ChunkHit, Document, Embedding, StorageAdapter};

// ── Public types ──────────────────────────────────────────────────────────────

/// A retrieved chunk with its RRF relevance score and citation metadata (FR-8).
#[derive(Debug, Clone)]
pub struct RetrievalResult {
    pub document_id: Uuid,
    /// Stable paragraph identifier — use this to cite the source (FR-8).
    pub paragraph_id: String,
    pub content: String,
    pub ordinal: u32,
    /// Reciprocal rank fusion score; higher means more relevant.
    pub score: f32,
}

// ── Chunking ──────────────────────────────────────────────────────────────────

/// Split a document body into paragraph chunks.
///
/// Paragraphs are delimited by one or more blank lines (`\n\n`). Whitespace-only
/// segments are discarded. Each chunk gets a stable `paragraph_id` derived from
/// its ordinal and content (see [`make_paragraph_id`]).
pub fn chunk_document(doc: &Document) -> Vec<Chunk> {
    doc.body
        .split("\n\n")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .enumerate()
        .map(|(i, content)| {
            let ordinal = i as u32;
            Chunk {
                document_id: doc.id,
                paragraph_id: make_paragraph_id(ordinal, content),
                content: content.to_string(),
                ordinal,
            }
        })
        .collect()
}

/// Derive a stable 16-character hex paragraph ID from its position and content.
///
/// `SHA-256(ordinal_le_bytes | content_utf8)`, truncated to 16 hex chars.
/// Changing either the ordinal or the content changes the ID, ensuring changed
/// paragraphs are re-embedded (FR-6 efficiency contract).
pub fn make_paragraph_id(ordinal: u32, content: &str) -> String {
    let mut h = Sha256::new();
    h.update(ordinal.to_le_bytes());
    h.update(content.as_bytes());
    let hex = format!("{:x}", h.finalize());
    hex[..16].to_string()
}

// ── Indexing ──────────────────────────────────────────────────────────────────

/// Index all chunks of a document, embedding only those that have changed.
///
/// # Re-embedding efficiency (FR-6)
/// For each chunk, the content hash is compared against the stored hash for the
/// same `paragraph_id`. A chunk triggers an embedding call only if:
/// - Its `paragraph_id` does not exist in storage (new paragraph), or
/// - Its content hash differs from the stored hash (edited paragraph).
///
/// # Returns
/// The number of embedding API calls made (0 if nothing changed).
pub async fn index_document_chunks(
    doc: &Document,
    storage: &Arc<dyn StorageAdapter>,
    router: &ModelRouter,
) -> Result<usize, KayaError> {
    let new_chunks = chunk_document(doc);

    // Existing paragraph_id → content_hash map for this document.
    let stored: HashMap<String, String> =
        storage.get_chunk_hashes(doc.id).await?.into_iter().collect();

    // Paragraph IDs that will exist after this re-index.
    let new_ids: std::collections::HashSet<String> =
        new_chunks.iter().map(|c| c.paragraph_id.clone()).collect();

    // Stale IDs: previously stored but no longer present → their embeddings
    // must be deleted so the vector index stays clean.
    let stale_ids: Vec<String> = stored
        .keys()
        .filter(|id| !new_ids.contains(*id))
        .cloned()
        .collect();

    // Chunks that need a fresh embedding (new or content changed).
    let to_embed: Vec<&Chunk> = new_chunks
        .iter()
        .filter(|chunk| {
            let hash = content_hash(&chunk.content);
            stored.get(&chunk.paragraph_id) != Some(&hash)
        })
        .collect();

    let embed_count = to_embed.len();

    // Generate all new embeddings BEFORE mutating storage.  If any embed call
    // fails, the existing index is left untouched.
    let mut new_embeddings: Vec<Embedding> = Vec::with_capacity(embed_count);
    for chunk in &to_embed {
        let resp = router.embed(&chunk.content).await?;
        new_embeddings.push(Embedding {
            document_id: chunk.document_id,
            paragraph_id: chunk.paragraph_id.clone(),
            vector: resp.embedding,
        });
    }

    // Remove stale vector embeddings.
    storage
        .delete_embeddings_for_paragraphs(doc.id, &stale_ids)
        .await?;

    // Rebuild the FTS5 + chunk metadata for this document.  We delete all and
    // re-insert because FTS5 doesn't support partial row replacement by
    // (document_id, paragraph_id) without a rowid look-up.
    storage.delete_chunks_for_document(doc.id).await?;
    for chunk in &new_chunks {
        storage.save_chunk(chunk).await?;
    }

    // Persist new embeddings.  Embeddings for unchanged paragraphs were not
    // deleted above and remain valid in chunk_embeddings.
    for emb in new_embeddings {
        storage.save_embeddings(&emb).await?;
    }

    Ok(embed_count)
}

// ── Retrieval ─────────────────────────────────────────────────────────────────

/// Retrieve the top-`k` relevant chunks for `query` using hybrid search.
///
/// # Steps
/// 1. Embed the query via the router's embedding model.
/// 2. Run cosine-similarity vector search and BM25 FTS5 search **in parallel**.
/// 3. Fuse the two ranked lists with Reciprocal Rank Fusion (k = 60).
/// 4. Return the top-`k` results with full citation metadata (FR-7, FR-8).
pub async fn retrieve(
    query: &str,
    k: usize,
    storage: &Arc<dyn StorageAdapter>,
    router: &ModelRouter,
) -> Result<Vec<RetrievalResult>, KayaError> {
    if query.trim().is_empty() || k == 0 {
        return Ok(vec![]);
    }

    let query_vec = router.embed(query).await?.embedding;

    // Over-fetch so RRF has enough candidates from both lists.
    let search_limit = (k * 3).max(20);

    let (vector_hits, bm25_hits) = tokio::try_join!(
        async {
            storage
                .search_embeddings(&query_vec, search_limit)
                .await
                .map_err(KayaError::from)
        },
        async {
            storage
                .search_text(query, search_limit)
                .await
                .map_err(KayaError::from)
        },
    )?;

    Ok(reciprocal_rank_fusion(&vector_hits, &bm25_hits, 60.0, k))
}

// ── RRF ───────────────────────────────────────────────────────────────────────

fn reciprocal_rank_fusion(
    vector_hits: &[ChunkHit],
    bm25_hits: &[ChunkHit],
    k_rrf: f32,
    top_k: usize,
) -> Vec<RetrievalResult> {
    // (document_id, paragraph_id) → (accumulated_score, hit_ref)
    let mut scores: HashMap<(Uuid, String), (f32, &ChunkHit)> = HashMap::new();

    for (rank, hit) in vector_hits.iter().enumerate() {
        let rrf = 1.0 / (k_rrf + (rank as f32 + 1.0));
        scores
            .entry((hit.document_id, hit.paragraph_id.clone()))
            .and_modify(|(s, _)| *s += rrf)
            .or_insert((rrf, hit));
    }

    for (rank, hit) in bm25_hits.iter().enumerate() {
        let rrf = 1.0 / (k_rrf + (rank as f32 + 1.0));
        scores
            .entry((hit.document_id, hit.paragraph_id.clone()))
            .and_modify(|(s, _)| *s += rrf)
            .or_insert((rrf, hit));
    }

    let mut results: Vec<RetrievalResult> = scores
        .into_values()
        .map(|(score, hit)| RetrievalResult {
            document_id: hit.document_id,
            paragraph_id: hit.paragraph_id.clone(),
            content: hit.content.clone(),
            ordinal: hit.ordinal,
            score,
        })
        .collect();

    results.sort_unstable_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(top_k);
    results
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn content_hash(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    format!("{:x}", h.finalize())
}

// ── Unit tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_doc(body: &str) -> Document {
        crate::storage::Document {
            id: Uuid::new_v4(),
            title: "Test".to_string(),
            owner: None,
            last_reviewed: None,
            tags: vec![],
            related_docs: vec![],
            body: body.to_string(),
            path: None,
        }
    }

    #[test]
    fn chunk_document_splits_on_blank_lines() {
        let doc = make_doc("Para one.\n\nPara two.\n\nPara three.");
        let chunks = chunk_document(&doc);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].content, "Para one.");
        assert_eq!(chunks[0].ordinal, 0);
        assert_eq!(chunks[2].content, "Para three.");
    }

    #[test]
    fn chunk_document_skips_blank_segments() {
        let doc = make_doc("Alpha.\n\n\n\nBeta.");
        let chunks = chunk_document(&doc);
        assert_eq!(chunks.len(), 2);
    }

    #[test]
    fn paragraph_id_is_stable_for_same_input() {
        let id1 = make_paragraph_id(0, "Hello world");
        let id2 = make_paragraph_id(0, "Hello world");
        assert_eq!(id1, id2);
    }

    #[test]
    fn paragraph_id_differs_on_content_change() {
        let id1 = make_paragraph_id(0, "Original text");
        let id2 = make_paragraph_id(0, "Edited text");
        assert_ne!(id1, id2);
    }

    #[test]
    fn paragraph_id_differs_on_ordinal_change() {
        let id1 = make_paragraph_id(0, "Same content");
        let id2 = make_paragraph_id(1, "Same content");
        assert_ne!(id1, id2);
    }

    #[test]
    fn rrf_scores_item_appearing_in_both_lists_higher() {
        let doc_id = Uuid::new_v4();
        let shared = ChunkHit {
            document_id: doc_id,
            paragraph_id: "shared".to_string(),
            content: "shared content".to_string(),
            ordinal: 0,
        };
        let only_vec = ChunkHit {
            document_id: doc_id,
            paragraph_id: "only_vec".to_string(),
            content: "vec only".to_string(),
            ordinal: 1,
        };
        let only_bm25 = ChunkHit {
            document_id: doc_id,
            paragraph_id: "only_bm25".to_string(),
            content: "bm25 only".to_string(),
            ordinal: 2,
        };

        let vector_hits = vec![shared.clone(), only_vec.clone()];
        let bm25_hits = vec![shared.clone(), only_bm25.clone()];

        let results = reciprocal_rank_fusion(&vector_hits, &bm25_hits, 60.0, 3);
        assert_eq!(results[0].paragraph_id, "shared", "shared item must rank first");
        assert!(results[0].score > results[1].score);
    }
}
