//! Integration tests for `SqliteAdapter`.
//!
//! Each test spins up a fresh temporary directory + SQLite database so the tests
//! are completely isolated and can run in parallel.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Instant;

use async_trait::async_trait;
use chrono::NaiveDate;
use futures::stream::BoxStream;
use uuid::Uuid;

use kaya_core::{
    KayaError, OperationType,
    model_router::{
        CompletionRequest, CompletionResponse, EmbeddingRequest, EmbeddingResponse, LlmProvider,
        ModelRouter, StreamItem, ToolCallRequest, ToolCallResponse,
        meter::TokenUsage,
    },
    retrieval::{chunk_document, index_document_chunks, retrieve},
    storage::{Document, StorageAdapter},
};
use kaya_storage::SqliteAdapter;

// ── Shared helpers ─────────────────────────────────────────────────────────────

fn temp_env() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let content = dir.path().join("content");
    let db = dir.path().join("index.db");
    (dir, content, db)
}

fn make_doc(rel_path: &str) -> Document {
    Document {
        id: Uuid::new_v4(),
        title: "Integration Test Doc".to_string(),
        owner: Some("alice".to_string()),
        last_reviewed: Some(NaiveDate::from_ymd_opt(2024, 6, 1).unwrap()),
        tags: vec!["rust".to_string(), "sqlite".to_string()],
        related_docs: vec![],
        body: "# Hello\n\nThis is the body.\n".to_string(),
        path: Some(PathBuf::from(rel_path)),
    }
}

fn blank_doc() -> Document {
    Document {
        id: Uuid::new_v4(),
        title: String::new(),
        owner: None,
        last_reviewed: None,
        tags: vec![],
        related_docs: vec![],
        body: String::new(),
        path: None,
    }
}

// ── Topic embedder ─────────────────────────────────────────────────────────────
//
// Returns deterministic 3-dim unit vectors based on topic keywords so that
// cosine similarity tests are fully deterministic without a real LLM.

struct TopicEmbedder {
    call_count: Arc<AtomicUsize>,
}

impl TopicEmbedder {
    fn new() -> (Arc<Self>, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0));
        (Arc::new(Self { call_count: Arc::clone(&count) }), count)
    }
}

fn topic_vector(text: &str) -> Vec<f32> {
    let t = text.to_lowercase();
    if t.contains("alpha") {
        vec![1.0, 0.0, 0.0]
    } else if t.contains("beta") {
        vec![0.0, 1.0, 0.0]
    } else if t.contains("gamma") {
        vec![0.0, 0.0, 1.0]
    } else {
        let v = 1.0_f32 / 3.0_f32.sqrt();
        vec![v, v, v]
    }
}

#[async_trait]
impl LlmProvider for TopicEmbedder {
    async fn complete(&self, _: CompletionRequest) -> Result<CompletionResponse, KayaError> {
        unreachable!("TopicEmbedder does not implement complete")
    }

    async fn stream(
        &self,
        _: CompletionRequest,
    ) -> Result<BoxStream<'static, Result<StreamItem, KayaError>>, KayaError> {
        unreachable!("TopicEmbedder does not implement stream")
    }

    async fn embed(&self, req: EmbeddingRequest) -> Result<EmbeddingResponse, KayaError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(EmbeddingResponse {
            embedding: topic_vector(&req.text),
            usage: TokenUsage {
                input_tokens: 1,
                output_tokens: 0,
                model: req.model,
                operation: OperationType::Embedding,
            },
        })
    }

    async fn tool_call(&self, _: ToolCallRequest) -> Result<ToolCallResponse, KayaError> {
        unreachable!("TopicEmbedder does not implement tool_call")
    }
}

fn make_router(embedder: Arc<dyn LlmProvider>) -> ModelRouter {
    let mut routes: HashMap<OperationType, (Arc<dyn LlmProvider>, String)> = HashMap::new();
    routes.insert(OperationType::Embedding, (embedder, "test-model".to_string()));
    ModelRouter::from_routes(routes)
}

// ── Document round-trip tests (Prompt 1) ──────────────────────────────────────

#[tokio::test]
async fn test_round_trip() {
    let (_dir, content, db) = temp_env();
    let adapter = SqliteAdapter::new(content, &db).await.unwrap();
    adapter.wait_for_reconciliation().await;

    let doc = make_doc("round_trip.md");
    adapter.save_document(&doc).await.unwrap();

    let loaded = adapter.get_document(doc.id).await.unwrap();
    assert_eq!(loaded.id, doc.id);
    assert_eq!(loaded.title, doc.title);
    assert_eq!(loaded.owner, doc.owner);
    assert_eq!(loaded.last_reviewed, doc.last_reviewed);
    assert_eq!(loaded.tags, doc.tags);
    assert_eq!(loaded.body.trim(), doc.body.trim());
    assert_eq!(loaded.path.as_deref(), Some(PathBuf::from("round_trip.md").as_path()));

    let all = adapter.list_documents().await.unwrap();
    assert!(all.iter().any(|d| d.id == doc.id));
}

#[tokio::test]
async fn test_manual_edit_detection() {
    let (_dir, content, db) = temp_env();

    {
        let adapter = SqliteAdapter::new(content.clone(), &db).await.unwrap();
        adapter.wait_for_reconciliation().await;
        let doc = make_doc("manual_edit.md");
        adapter.save_document(&doc).await.unwrap();
    }

    let file_path = content.join("manual_edit.md");
    let original_raw = std::fs::read_to_string(&file_path).unwrap();
    let patched = original_raw.replace("title: Integration Test Doc", "title: Edited Title");
    std::fs::write(&file_path, &patched).unwrap();

    let adapter2 = SqliteAdapter::new(content.clone(), &db).await.unwrap();
    adapter2.wait_for_reconciliation().await;

    let (doc_from_file, _) = kaya_storage::document::parse_document(&patched).unwrap();
    let loaded = adapter2.get_document(doc_from_file.id).await.unwrap();
    assert_eq!(loaded.title, "Edited Title");
}

#[tokio::test]
async fn test_uuid_stability_after_rename() {
    let (_dir, content, db) = temp_env();
    let adapter = SqliteAdapter::new(content.clone(), &db).await.unwrap();
    adapter.wait_for_reconciliation().await;

    let doc = make_doc("original_name.md");
    let original_id = doc.id;
    adapter.save_document(&doc).await.unwrap();

    std::fs::rename(content.join("original_name.md"), content.join("renamed.md")).unwrap();

    let adapter2 = SqliteAdapter::new(content.clone(), &db).await.unwrap();
    adapter2.wait_for_reconciliation().await;

    let mut renamed_doc = doc.clone();
    renamed_doc.path = Some(PathBuf::from("renamed.md"));
    adapter2.save_document(&renamed_doc).await.unwrap();

    let loaded = adapter2.get_document(original_id).await.unwrap();
    assert_eq!(loaded.id, original_id);
    assert_eq!(loaded.path.as_deref(), Some(PathBuf::from("renamed.md").as_path()));
}

#[tokio::test]
async fn test_file_on_disk_wins() {
    let (_dir, content, db) = temp_env();
    let adapter = SqliteAdapter::new(content.clone(), &db).await.unwrap();
    adapter.wait_for_reconciliation().await;

    let doc = make_doc("truth.md");
    adapter.save_document(&doc).await.unwrap();

    {
        use sqlx::sqlite::SqliteConnectOptions;
        use sqlx::SqlitePool;
        let opts = SqliteConnectOptions::new().filename(&db);
        let pool = SqlitePool::connect_with(opts).await.unwrap();
        sqlx::query(
            "UPDATE documents SET frontmatter_json = '{\"corrupted\":true}' WHERE id = ?",
        )
        .bind(doc.id.to_string())
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;
    }

    let loaded = adapter.get_document(doc.id).await.unwrap();
    assert_eq!(loaded.title, doc.title);
    assert_eq!(loaded.body.trim(), doc.body.trim());
}

// ── Retrieval tests (Prompt 2) ─────────────────────────────────────────────────

/// FR-7: Hybrid retrieval over a seed corpus returns the expected top result.
///
/// Three documents are seeded with distinct topic keywords (alpha, beta, gamma).
/// `TopicEmbedder` returns a unit vector per topic so cosine similarity is 1.0
/// for a matching query and 0.0 for others. FTS5 BM25 likewise gives the
/// matching document the top rank. After RRF fusion, the correct document wins.
#[tokio::test]
async fn test_retrieval_seed_corpus() {
    let (_dir, content, db) = temp_env();
    let adapter = Arc::new(SqliteAdapter::new(content.clone(), &db).await.unwrap());
    adapter.wait_for_reconciliation().await;

    let (embedder, _count) = TopicEmbedder::new();
    let router = make_router(embedder);

    let doc_a = Document {
        id: Uuid::new_v4(),
        title: "Alpha Systems".to_string(),
        body: "Alpha particles are a type of ionizing radiation.\n\nAlpha decay releases helium nuclei.".to_string(),
        path: Some(PathBuf::from("alpha.md")),
        ..blank_doc()
    };
    let doc_b = Document {
        id: Uuid::new_v4(),
        title: "Beta Testing".to_string(),
        body: "Beta testing involves systematic verification.\n\nBeta releases precede stable versions.".to_string(),
        path: Some(PathBuf::from("beta.md")),
        ..blank_doc()
    };
    let doc_c = Document {
        id: Uuid::new_v4(),
        title: "Gamma Radiation".to_string(),
        body: "Gamma rays are electromagnetic waves of high frequency.\n\nGamma radiation penetrates most materials.".to_string(),
        path: Some(PathBuf::from("gamma.md")),
        ..blank_doc()
    };

    let storage: Arc<dyn StorageAdapter> = adapter;
    for doc in [&doc_a, &doc_b, &doc_c] {
        storage.save_document(doc).await.unwrap();
        index_document_chunks(doc, &storage, &router).await.unwrap();
    }

    let results = retrieve("alpha", 3, &storage, &router).await.unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].document_id, doc_a.id, "alpha query → alpha doc");

    let results = retrieve("beta", 3, &storage, &router).await.unwrap();
    assert_eq!(results[0].document_id, doc_b.id, "beta query → beta doc");

    let results = retrieve("gamma", 3, &storage, &router).await.unwrap();
    assert_eq!(results[0].document_id, doc_c.id, "gamma query → gamma doc");
}

/// FR-8: A retrieved chunk resolves back to the correct paragraph in the source.
#[tokio::test]
async fn test_citation_round_trip() {
    let (_dir, content, db) = temp_env();
    let adapter = Arc::new(SqliteAdapter::new(content.clone(), &db).await.unwrap());
    adapter.wait_for_reconciliation().await;

    let (embedder, _) = TopicEmbedder::new();
    let router = make_router(embedder);

    let doc = Document {
        id: Uuid::new_v4(),
        title: "Citation Test".to_string(),
        body: "First paragraph about alpha concepts.\n\nSecond paragraph discusses other topics.\n\nThird paragraph mentions gamma radiation.".to_string(),
        path: Some(PathBuf::from("citation.md")),
        ..blank_doc()
    };

    let storage: Arc<dyn StorageAdapter> = adapter;
    storage.save_document(&doc).await.unwrap();
    index_document_chunks(&doc, &storage, &router).await.unwrap();

    let results = retrieve("alpha", 1, &storage, &router).await.unwrap();
    assert!(!results.is_empty(), "retrieve must return a result");

    let hit = &results[0];
    assert_eq!(hit.document_id, doc.id, "citation points to correct document");

    // The cited paragraph_id must exist in the document's chunks.
    let all_chunks = chunk_document(&doc);
    let source_chunk = all_chunks
        .iter()
        .find(|c| c.paragraph_id == hit.paragraph_id)
        .expect("cited paragraph_id must exist in the source document");

    assert_eq!(source_chunk.content, hit.content, "chunk content must match");
}

/// FR-6: Editing one paragraph in a 10-paragraph document triggers exactly
/// one embedding call on re-index; the other 9 embeddings are reused.
#[tokio::test]
async fn test_reembedding_efficiency() {
    let (_dir, content, db) = temp_env();
    let adapter = Arc::new(SqliteAdapter::new(content.clone(), &db).await.unwrap());
    adapter.wait_for_reconciliation().await;

    let (embedder, call_count) = TopicEmbedder::new();
    let router = make_router(embedder);

    // Ten paragraphs, separated by double newlines.
    let make_body = |edit: bool| -> String {
        (0..10_usize)
            .map(|i| {
                if edit && i == 4 {
                    "Paragraph 4: EDITED content about beta.".to_string()
                } else {
                    format!("Paragraph {i}: content about alpha topic {i}.")
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    let doc = Document {
        id: Uuid::new_v4(),
        title: "Efficiency Test".to_string(),
        body: make_body(false),
        path: Some(PathBuf::from("efficiency.md")),
        ..blank_doc()
    };

    let storage: Arc<dyn StorageAdapter> = adapter;
    storage.save_document(&doc).await.unwrap();
    let first_embed_calls = index_document_chunks(&doc, &storage, &router).await.unwrap();
    assert_eq!(first_embed_calls, 10, "first index: all 10 paragraphs embedded");
    assert_eq!(call_count.load(Ordering::SeqCst), 10);

    // Edit paragraph 4 only.
    let edited_doc = Document { body: make_body(true), ..doc.clone() };
    let second_embed_calls =
        index_document_chunks(&edited_doc, &storage, &router).await.unwrap();

    assert_eq!(
        second_embed_calls, 1,
        "re-index after single edit must make exactly 1 embedding call"
    );
    assert_eq!(call_count.load(Ordering::SeqCst), 11);
}

/// NFR §6.1 (adapted): retrieval over a 100-document corpus (500 chunks) must
/// complete in under 200 ms on a development machine.
#[tokio::test]
async fn test_performance_smoke() {
    let (_dir, content, db) = temp_env();
    let adapter = Arc::new(SqliteAdapter::new(content.clone(), &db).await.unwrap());
    adapter.wait_for_reconciliation().await;

    let (embedder, _) = TopicEmbedder::new();
    let router = make_router(embedder);
    let storage: Arc<dyn StorageAdapter> = adapter;

    // Seed 100 documents with 5 paragraphs each (500 chunks + 500 embeddings).
    let topics = ["alpha", "beta", "gamma"];
    for i in 0..100_usize {
        let topic = topics[i % 3];
        let body = (0..5)
            .map(|p| {
                format!("Document {i} paragraph {p}: discusses {topic} concepts in depth.")
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let doc = Document {
            id: Uuid::new_v4(),
            title: format!("Document {i}"),
            body,
            path: Some(PathBuf::from(format!("doc_{i:03}.md"))),
            ..blank_doc()
        };

        storage.save_document(&doc).await.unwrap();
        index_document_chunks(&doc, &storage, &router).await.unwrap();
    }

    let start = Instant::now();
    let results = retrieve("alpha concepts", 5, &storage, &router).await.unwrap();
    let elapsed = start.elapsed();

    assert!(!results.is_empty(), "retrieval must return results");
    assert!(
        elapsed.as_millis() < 200,
        "retrieval over 100-doc corpus took {}ms, expected < 200ms",
        elapsed.as_millis()
    );
}
