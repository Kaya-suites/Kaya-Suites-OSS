//! CLI harness for the Kaya agent loop.
//!
//! Runs one agent turn against a mock in-memory knowledge base, prints each
//! event as it arrives, and shows the propose-then-approve flow end-to-end.
//!
//! ```
//! cargo run -p kaya-core --example agent_cli
//! ```

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
    CompletionRequest, CompletionResponse, EmbeddingRequest, EmbeddingResponse,
    LlmProvider, ModelRouter, OperationType, StreamItem, ToolCallRequest, ToolCallResponse,
    ToolCallResult, TokenUsage,
};
use kaya_core::storage::{Document, Embedding, StorageAdapter, StorageError};

// ── Minimal in-memory storage ────────────────────────────────────────────────

struct Mem(Arc<Mutex<HashMap<Uuid, Document>>>);

impl Mem {
    fn new(docs: Vec<Document>) -> Arc<Self> {
        let map: HashMap<_, _> = docs.into_iter().map(|d| (d.id, d)).collect();
        Arc::new(Self(Arc::new(Mutex::new(map))))
    }
}

#[async_trait]
impl StorageAdapter for Mem {
    async fn get_document(&self, id: Uuid) -> Result<Document, StorageError> {
        self.0.lock().unwrap().get(&id).cloned().ok_or(StorageError::NotFound(id))
    }
    async fn save_document(&self, doc: &Document) -> Result<(), StorageError> {
        self.0.lock().unwrap().insert(doc.id, doc.clone());
        Ok(())
    }
    async fn delete_document(&self, id: Uuid) -> Result<(), StorageError> {
        self.0.lock().unwrap().remove(&id);
        Ok(())
    }
    async fn list_documents(&self) -> Result<Vec<Document>, StorageError> {
        Ok(self.0.lock().unwrap().values().cloned().collect())
    }
    async fn search_embeddings(&self, _q: &[f32], _lim: usize) -> Result<Vec<Embedding>, StorageError> {
        Ok(vec![])
    }
    async fn save_embeddings(&self, _e: &Embedding) -> Result<(), StorageError> {
        Ok(())
    }
}

// ── Scripted mock provider ───────────────────────────────────────────────────

struct MockProvider {
    turns: Mutex<std::collections::VecDeque<(Option<ToolCallResult>, Option<String>)>>,
}

impl MockProvider {
    fn new(turns: Vec<(Option<ToolCallResult>, Option<String>)>) -> Arc<Self> {
        Arc::new(Self { turns: Mutex::new(turns.into()) })
    }
}

#[async_trait]
impl LlmProvider for MockProvider {
    async fn complete(&self, r: CompletionRequest) -> Result<CompletionResponse, KayaError> {
        Ok(CompletionResponse { content: String::new(), usage: usage(r.model, r.operation) })
    }
    async fn stream(&self, r: CompletionRequest) -> Result<futures::stream::BoxStream<'static, Result<StreamItem, KayaError>>, KayaError> {
        Ok(Box::pin(futures::stream::iter(vec![Ok(StreamItem::Usage(usage(r.model, r.operation)))])))
    }
    async fn embed(&self, r: EmbeddingRequest) -> Result<EmbeddingResponse, KayaError> {
        Ok(EmbeddingResponse {
            embedding: vec![0.0; 3],
            usage: TokenUsage { input_tokens: 0, output_tokens: 0, model: r.model, operation: OperationType::Embedding },
        })
    }
    async fn tool_call(&self, r: ToolCallRequest) -> Result<ToolCallResponse, KayaError> {
        let (tc, content) = self.turns.lock().unwrap().pop_front()
            .expect("mock ran out of scripted turns");
        Ok(ToolCallResponse { result: tc, content, usage: usage(r.model, r.operation) })
    }
}

fn usage(model: String, operation: OperationType) -> TokenUsage {
    TokenUsage { input_tokens: 5, output_tokens: 10, model, operation }
}

fn router(p: Arc<dyn LlmProvider>) -> Arc<ModelRouter> {
    let mut routes = HashMap::new();
    for op in [
        OperationType::RetrievalClassification,
        OperationType::DocumentGeneration,
        OperationType::EditProposal,
        OperationType::StaleDetection,
        OperationType::Embedding,
    ] {
        routes.insert(op, (p.clone(), "mock-model".to_owned()));
    }
    Arc::new(ModelRouter::from_routes(routes))
}

// ── main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    // Seed the knowledge base with one document.
    let doc_id = Uuid::new_v4();
    let doc = Document {
        id: doc_id,
        title: "Onboarding Guide".into(),
        owner: Some("alice@example.com".into()),
        last_reviewed: None,
        tags: vec!["onboarding".into()],
        related_docs: vec![],
        body: "Welcome to the team.\n\nThis guide covers your first week.".into(),
        path: None,
    };
    let storage = Mem::new(vec![doc]);

    // Script the model: search → propose_edit → final answer.
    let provider = MockProvider::new(vec![
        (
            Some(ToolCallResult {
                tool_name: "search_documents".into(),
                arguments: json!({ "query": "onboarding", "limit": 3 }),
            }),
            None,
        ),
        (
            Some(ToolCallResult {
                tool_name: "propose_edit".into(),
                arguments: json!({
                    "document_id": doc_id.to_string(),
                    "new_body": "Welcome to the team.\n\nThis guide covers your first week.\n\nCheck the wiki for more resources.",
                    "reason": "Added wiki link paragraph"
                }),
            }),
            None,
        ),
        (
            None,
            Some("I've proposed adding a wiki link to the onboarding guide. Please review and approve.".into()),
        ),
    ]);

    let session = UserSession { user_id: Uuid::new_v4() };
    let ctx = Arc::new(AgentContext {
        storage: storage.clone() as Arc<dyn StorageAdapter>,
        router: router(provider as Arc<dyn LlmProvider>),
        session: session.clone(),
    });

    let log = Arc::new(InvocationLog::new());
    let agent = AgentLoop::new(default_tools());

    println!("── Agent turn ─────────────────────────────────────────────────");
    println!("User: Update the onboarding guide.\n");

    let mut stream = agent.run("Update the onboarding guide.".into(), ctx, log.clone());
    let mut proposed_edit = None;

    while let Some(ev) = stream.next().await {
        match ev.unwrap() {
            AgentEvent::ToolCall { name, input } => {
                println!("[tool call]  {name}({})", serde_json::to_string(&input).unwrap());
            }
            AgentEvent::ToolResult { name, output, latency_ms } => {
                println!("[tool result] {name} — {latency_ms}ms → {output}");
            }
            AgentEvent::ProposedEditEmitted { edit } => {
                println!("[proposed edit] id={}", edit.id);
                proposed_edit = Some(edit);
            }
            AgentEvent::FinalMessage { text } => {
                println!("\nAssistant: {text}");
            }
            AgentEvent::ThinkingChunk { text } => print!("{text}"),
        }
    }

    // ── Show invocation log ───────────────────────────────────────────────────
    println!("\n── Invocation log ({} entries) ─────────────────────────────", log.len());
    for entry in log.entries() {
        let status = if entry.output.is_ok() { "ok" } else { "err" };
        println!("  {} | {} | {}ms | {}", entry.turn_id, entry.tool_name, entry.latency_ms, status);
    }

    // ── Approve and commit ────────────────────────────────────────────────────
    if let Some(edit) = proposed_edit {
        println!("\n── Approving edit {} ────────────────────────────────────────", edit.id);
        let token = session.approve_edit(&edit);
        commit_edit(edit, token, storage.clone() as Arc<dyn StorageAdapter>)
            .await
            .expect("commit_edit failed");

        let updated = storage.get_document(doc_id).await.unwrap();
        println!("Document body after commit:\n{}", updated.body);
    }
}
