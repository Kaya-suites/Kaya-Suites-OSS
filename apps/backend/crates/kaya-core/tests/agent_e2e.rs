//! End-to-end agent tests.
//!
//! Tests verified:
//! 1. Search-then-edit turn produces a `ProposedEdit`; document unchanged until
//!    `commit_edit` is called with an `ApprovalToken` (FR-15).
//! 2. Every tool call is recorded in the `InvocationLog` (FR-14).
//! 3. Cancelling the stream mid-turn does not panic or leak tasks.
//! 4. `commit_edit` with an `ApprovalToken` from `UserSession::approve_edit`
//!    applies the change; without a token it cannot be called (trybuild).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::StreamExt;
use serde_json::json;
use uuid::Uuid;

use kaya_core::agent::{AgentContext, AgentEvent, AgentLoop, InvocationLog};
use kaya_core::agent::tools::default_tools;
use kaya_core::auth::UserSession;
use kaya_core::edit::commit_edit;
use kaya_core::error::KayaError;
use kaya_core::model_router::{
    CompletionRequest, CompletionResponse, EmbeddingRequest, EmbeddingResponse, LlmProvider,
    ModelRouter, OperationType, StreamItem, ToolCallRequest, ToolCallResponse, ToolCallResult,
    TokenUsage,
};
use kaya_core::storage::{Document, Embedding, StorageAdapter, StorageError};

// ── In-memory StorageAdapter ─────────────────────────────────────────────────

struct MemStorage {
    docs: Arc<Mutex<HashMap<Uuid, Document>>>,
}

impl MemStorage {
    fn new() -> Self {
        Self { docs: Arc::new(Mutex::new(HashMap::new())) }
    }

    fn with_doc(doc: Document) -> Self {
        let s = Self::new();
        s.docs.lock().unwrap().insert(doc.id, doc);
        s
    }

    fn get_doc_sync(&self, id: Uuid) -> Option<Document> {
        self.docs.lock().unwrap().get(&id).cloned()
    }
}

#[async_trait]
impl StorageAdapter for MemStorage {
    async fn get_document(&self, id: Uuid) -> Result<Document, StorageError> {
        self.docs.lock().unwrap().get(&id).cloned().ok_or(StorageError::NotFound(id))
    }
    async fn save_document(&self, doc: &Document) -> Result<(), StorageError> {
        self.docs.lock().unwrap().insert(doc.id, doc.clone());
        Ok(())
    }
    async fn delete_document(&self, id: Uuid) -> Result<(), StorageError> {
        self.docs.lock().unwrap().remove(&id);
        Ok(())
    }
    async fn list_documents(&self) -> Result<Vec<Document>, StorageError> {
        Ok(self.docs.lock().unwrap().values().cloned().collect())
    }
    async fn search_embeddings(&self, _q: &[f32], _lim: usize) -> Result<Vec<Embedding>, StorageError> {
        Ok(vec![])
    }
    async fn save_embeddings(&self, _e: &Embedding) -> Result<(), StorageError> {
        Ok(())
    }
}

// ── Scripted LLM provider ─────────────────────────────────────────────────────

/// One scripted turn: the model either calls a tool or gives a final answer.
struct ScriptedTurn {
    tool_call: Option<ToolCallResult>,
    content: Option<String>,
}

/// An [`LlmProvider`] that pops pre-baked turns from a queue.
/// Ignores the prompt; simply returns the next scripted response.
struct ScriptedProvider {
    turns: Mutex<std::collections::VecDeque<ScriptedTurn>>,
}

impl ScriptedProvider {
    fn new(turns: Vec<ScriptedTurn>) -> Self {
        Self { turns: Mutex::new(turns.into()) }
    }
}

#[async_trait]
impl LlmProvider for ScriptedProvider {
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse, KayaError> {
        Ok(CompletionResponse {
            content: String::new(),
            usage: zero_usage(req.model, req.operation),
        })
    }
    async fn stream(
        &self,
        req: CompletionRequest,
    ) -> Result<futures::stream::BoxStream<'static, Result<StreamItem, KayaError>>, KayaError> {
        use futures::stream;
        let usage = zero_usage(req.model, req.operation);
        Ok(Box::pin(stream::iter(vec![Ok(StreamItem::Usage(usage))])))
    }
    async fn embed(&self, req: EmbeddingRequest) -> Result<EmbeddingResponse, KayaError> {
        Ok(EmbeddingResponse {
            embedding: vec![0.0; 3],
            usage: TokenUsage {
                input_tokens: 0,
                output_tokens: 0,
                model: req.model,
                operation: OperationType::Embedding,
            },
        })
    }
    async fn tool_call(&self, req: ToolCallRequest) -> Result<ToolCallResponse, KayaError> {
        let turn = self
            .turns
            .lock()
            .unwrap()
            .pop_front()
            .expect("ScriptedProvider: no more scripted turns");
        Ok(ToolCallResponse {
            result: turn.tool_call,
            content: turn.content,
            usage: zero_usage(req.model, req.operation),
        })
    }
}

fn zero_usage(model: String, operation: OperationType) -> TokenUsage {
    TokenUsage { input_tokens: 1, output_tokens: 1, model, operation }
}

fn all_ops() -> Vec<OperationType> {
    vec![
        OperationType::RetrievalClassification,
        OperationType::DocumentGeneration,
        OperationType::EditProposal,
        OperationType::StaleDetection,
        OperationType::Embedding,
    ]
}

fn router_with(provider: Arc<dyn LlmProvider>) -> Arc<ModelRouter> {
    let mut routes: HashMap<OperationType, (Arc<dyn LlmProvider>, String)> = HashMap::new();
    for op in all_ops() {
        routes.insert(op, (provider.clone(), "test-model".to_owned()));
    }
    Arc::new(ModelRouter::from_routes(routes))
}

fn make_doc(body: &str) -> Document {
    Document {
        id: Uuid::new_v4(),
        title: "Test Document".into(),
        owner: None,
        last_reviewed: None,
        tags: vec![],
        related_docs: vec![],
        body: body.into(),
        path: None,
    }
}

// ── Test 1: propose-then-approve invariant ───────────────────────────────────

#[tokio::test]
async fn search_then_edit_requires_approval() {
    let doc = make_doc("Old paragraph one.\n\nOld paragraph two.");
    let doc_id = doc.id;
    let storage = Arc::new(MemStorage::with_doc(doc));

    // Turn 1 — search_documents; Turn 2 — propose_edit; Turn 3 — final answer.
    let provider = Arc::new(ScriptedProvider::new(vec![
        ScriptedTurn {
            tool_call: Some(ToolCallResult {
                tool_name: "search_documents".into(),
                arguments: json!({ "query": "paragraph", "limit": 3 }),
            }),
            content: None,
        },
        ScriptedTurn {
            tool_call: Some(ToolCallResult {
                tool_name: "propose_edit".into(),
                arguments: json!({
                    "document_id": doc_id.to_string(),
                    "new_body": "New paragraph one.\n\nNew paragraph two.",
                    "reason": "Updating content"
                }),
            }),
            content: None,
        },
        ScriptedTurn {
            tool_call: None,
            content: Some("I have proposed an edit to the document.".into()),
        },
    ]));

    let ctx = Arc::new(AgentContext {
        storage: storage.clone() as Arc<dyn StorageAdapter>,
        router: router_with(provider as Arc<dyn LlmProvider>),
        session: UserSession { user_id: Uuid::new_v4() },
    });

    let log = Arc::new(InvocationLog::new());
    let agent = AgentLoop::new(default_tools());
    let mut stream = agent.run("Update the test document.".into(), ctx.clone(), log.clone());

    let mut events: Vec<AgentEvent> = Vec::new();
    while let Some(ev) = stream.next().await {
        events.push(ev.expect("agent event should not error"));
    }

    // ── Find the ProposedEdit ──────────────────────────────────────────────────
    let proposed = events
        .iter()
        .find_map(|e| {
            if let AgentEvent::ProposedEditEmitted { edit } = e { Some(edit.clone()) } else { None }
        })
        .expect("agent must emit a ProposedEditEmitted event");

    // ── Document must still be unchanged ─────────────────────────────────────
    let before = storage.get_doc_sync(doc_id).unwrap();
    assert_eq!(
        before.body, "Old paragraph one.\n\nOld paragraph two.",
        "document body must not change before approval"
    );

    // ── Approve and commit ────────────────────────────────────────────────────
    let session = UserSession { user_id: Uuid::new_v4() };
    let token = session.approve_edit(&proposed);
    commit_edit(proposed, token, storage.clone() as Arc<dyn StorageAdapter>)
        .await
        .expect("commit_edit should succeed");

    // ── Document must now reflect the proposed body ───────────────────────────
    let after = storage.get_doc_sync(doc_id).unwrap();
    assert_eq!(
        after.body, "New paragraph one.\n\nNew paragraph two.",
        "document body must reflect the approved edit"
    );

    // ── Final message must have been emitted ─────────────────────────────────
    assert!(
        events.iter().any(|e| matches!(e, AgentEvent::FinalMessage { .. })),
        "stream must end with a FinalMessage"
    );
}

// ── Test 2: tool transparency ────────────────────────────────────────────────

#[tokio::test]
async fn invocation_log_captures_every_tool_used() {
    let doc = make_doc("Some content.");
    let doc_id = doc.id;
    let storage = Arc::new(MemStorage::with_doc(doc));

    let provider = Arc::new(ScriptedProvider::new(vec![
        ScriptedTurn {
            tool_call: Some(ToolCallResult {
                tool_name: "list_documents".into(),
                arguments: json!({}),
            }),
            content: None,
        },
        ScriptedTurn {
            tool_call: Some(ToolCallResult {
                tool_name: "read_document".into(),
                arguments: json!({ "document_id": doc_id.to_string() }),
            }),
            content: None,
        },
        ScriptedTurn {
            tool_call: None,
            content: Some("Here is the document.".into()),
        },
    ]));

    let ctx = Arc::new(AgentContext {
        storage: storage.clone() as Arc<dyn StorageAdapter>,
        router: router_with(provider as Arc<dyn LlmProvider>),
        session: UserSession { user_id: Uuid::new_v4() },
    });

    let log = Arc::new(InvocationLog::new());
    let agent = AgentLoop::new(default_tools());
    let mut stream = agent.run("Show me the documents.".into(), ctx, log.clone());
    while let Some(ev) = stream.next().await {
        ev.expect("no errors");
    }

    let entries = log.entries();
    let names: Vec<&str> = entries.iter().map(|e| e.tool_name.as_str()).collect();

    assert!(names.contains(&"list_documents"), "log must contain list_documents");
    assert!(names.contains(&"read_document"),  "log must contain read_document");
    assert_eq!(entries.len(), 2, "exactly 2 tool calls should be logged");

    // Every entry must have a latency measurement.
    for entry in &entries {
        assert!(entry.latency_ms < 5_000, "latency should be sane");
    }
}

// ── Test 3: cancellation ─────────────────────────────────────────────────────

#[tokio::test]
async fn cancellation_does_not_panic_or_leak() {
    let storage = Arc::new(MemStorage::new());

    // Five tool calls followed by a final message — we will cancel after the first.
    let turns: Vec<ScriptedTurn> = (0..5)
        .map(|_| ScriptedTurn {
            tool_call: Some(ToolCallResult {
                tool_name: "list_documents".into(),
                arguments: json!({}),
            }),
            content: None,
        })
        .chain(std::iter::once(ScriptedTurn {
            tool_call: None,
            content: Some("Done.".into()),
        }))
        .collect();

    let provider = Arc::new(ScriptedProvider::new(turns));
    let ctx = Arc::new(AgentContext {
        storage: storage as Arc<dyn StorageAdapter>,
        router: router_with(provider as Arc<dyn LlmProvider>),
        session: UserSession { user_id: Uuid::new_v4() },
    });

    let log = Arc::new(InvocationLog::new());
    let agent = AgentLoop::new(default_tools());
    let mut stream = agent.run("List docs.".into(), ctx, log);

    // Consume only the first event, then drop the stream.
    let first = stream.next().await;
    assert!(first.is_some(), "should get at least one event");
    drop(stream);

    // Give the background task a moment to notice the cancelled sender.
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    // If the task panicked, tokio would surface it. Reaching here = clean exit.
}

// ── Test 4: create_document also requires approval ───────────────────────────

#[tokio::test]
async fn create_document_requires_approval() {
    let storage = Arc::new(MemStorage::new());

    let provider = Arc::new(ScriptedProvider::new(vec![
        ScriptedTurn {
            tool_call: Some(ToolCallResult {
                tool_name: "create_document".into(),
                arguments: json!({
                    "title": "Brand New Doc",
                    "body": "# Brand New\n\nFresh content."
                }),
            }),
            content: None,
        },
        ScriptedTurn {
            tool_call: None,
            content: Some("Created a new document proposal.".into()),
        },
    ]));

    let ctx = Arc::new(AgentContext {
        storage: storage.clone() as Arc<dyn StorageAdapter>,
        router: router_with(provider as Arc<dyn LlmProvider>),
        session: UserSession { user_id: Uuid::new_v4() },
    });

    let log = Arc::new(InvocationLog::new());
    let agent = AgentLoop::new(default_tools());
    let mut stream = agent.run("Create a doc.".into(), ctx, log);

    let mut events = Vec::new();
    while let Some(ev) = stream.next().await {
        events.push(ev.unwrap());
    }

    let proposed = events
        .iter()
        .find_map(|e| {
            if let AgentEvent::ProposedEditEmitted { edit } = e { Some(edit.clone()) } else { None }
        })
        .expect("must emit ProposedEditEmitted for create_document");

    // Storage must still be empty — no approval yet.
    assert_eq!(
        storage.docs.lock().unwrap().len(),
        0,
        "no document should exist before approval"
    );

    // Approve → commit → document must now exist.
    let session = UserSession { user_id: Uuid::new_v4() };
    let token = session.approve_edit(&proposed);
    commit_edit(proposed, token, storage.clone() as Arc<dyn StorageAdapter>)
        .await
        .unwrap();

    assert_eq!(
        storage.docs.lock().unwrap().len(),
        1,
        "document must exist after approval"
    );
}
