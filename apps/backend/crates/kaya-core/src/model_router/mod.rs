//! LLM provider abstraction, routing, and token metering.
//!
//! # Routing configuration (`kaya.yaml`)
//!
//! ```yaml
//! routing:
//!   retrieval_classification:
//!     provider: openai
//!     model: gpt-4o-mini
//!   document_generation:
//!     provider: anthropic
//!     model: claude-opus-4-6
//!   edit_proposal:
//!     provider: anthropic
//!     model: claude-opus-4-6
//!   stale_detection:
//!     provider: openai
//!     model: gpt-4o-mini
//!   embedding:
//!     provider: openai
//!     model: text-embedding-3-small
//!
//! providers:
//!   openai:
//!     api_key_env: OPENAI_API_KEY
//!   anthropic:
//!     api_key_env: ANTHROPIC_API_KEY
//! ```
//!
//! # Rule
//!
//! **No code outside a provider implementation file (`anthropic.rs`, `openai.rs`) may
//! import a vendor SDK.** All LLM calls in business logic must go through
//! [`LlmProvider`] or [`ModelRouter`].

pub mod config;
pub mod meter;
pub mod router;

mod anthropic;
mod openai;

#[cfg(test)]
pub mod mock;

pub use config::ConfigError;
pub use meter::{Meter, TokenUsage};
pub use router::ModelRouter;

use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};

use crate::error::KayaError;

// ---- Operation type -------------------------------------------------------

/// The logical operation for which an LLM call is being made.
///
/// The routing table maps each variant to a `(provider, model)` pair.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    /// Classify retrieved documents (fast model).
    RetrievalClassification,
    /// Generate new document content (strong model).
    DocumentGeneration,
    /// Propose an edit to an existing document (strong model).
    EditProposal,
    /// Detect whether a document is stale (fast model).
    StaleDetection,
    /// Embed text for vector search (dedicated embedding model).
    Embedding,
}

// ---- Request / response types ---------------------------------------------

#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub prompt: String,
    pub model: String,
    pub operation: OperationType,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct CompletionResponse {
    pub content: String,
    pub usage: TokenUsage,
}

/// A single incremental text delta from a streaming response.
#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub delta: String,
}

/// An item yielded from a streaming completion.
#[derive(Debug, Clone)]
pub enum StreamItem {
    /// Incremental text chunk.
    Chunk(StreamChunk),
    /// Final token-usage summary. Always the last item in a successful stream.
    Usage(TokenUsage),
}

#[derive(Debug, Clone)]
pub struct EmbeddingRequest {
    pub text: String,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct EmbeddingResponse {
    pub embedding: Vec<f32>,
    pub usage: TokenUsage,
}

/// A structured tool/function definition passed to the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    /// JSON Schema describing the tool's parameters.
    pub parameters: serde_json::Value,
}

/// The result of a model-chosen tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub tool_name: String,
    /// Raw JSON arguments chosen by the model.
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ToolCallRequest {
    pub prompt: String,
    pub model: String,
    pub operation: OperationType,
    pub tools: Vec<ToolDefinition>,
}

#[derive(Debug, Clone)]
pub struct ToolCallResponse {
    pub result: Option<ToolCallResult>,
    /// Text content from the model when it chose *not* to call a tool.
    /// This is the final answer the agent should surface as a [`FinalMessage`].
    pub content: Option<String>,
    pub usage: TokenUsage,
}

// ---- Trait ----------------------------------------------------------------

/// Abstracts over LLM vendors (Anthropic, OpenAI, …).
///
/// **No vendor SDK may be imported outside of the concrete implementation
/// files.** All LLM calls in business logic must go through this trait or
/// [`ModelRouter`].
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Single-turn completion; returns the full response text.
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, KayaError>;

    /// Streaming completion.
    ///
    /// The returned stream yields [`StreamItem::Chunk`] items followed by a
    /// single [`StreamItem::Usage`] as the final item. Dropping the stream
    /// before exhaustion cancels it cleanly.
    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> Result<BoxStream<'static, Result<StreamItem, KayaError>>, KayaError>;

    /// Generate a vector embedding for `request.text`.
    async fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, KayaError>;

    /// Single-turn tool call; returns the model's chosen tool invocation if any.
    async fn tool_call(&self, request: ToolCallRequest) -> Result<ToolCallResponse, KayaError>;
}

// ---- Tests ----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use futures::StreamExt;

    use super::mock::MockProvider;
    use super::*;

    fn router_with_mock(mock: Arc<MockProvider>) -> ModelRouter {
        let mut routes: HashMap<OperationType, (Arc<dyn LlmProvider>, String)> = HashMap::new();
        for (op, model) in [
            (OperationType::RetrievalClassification, "fast-model"),
            (OperationType::DocumentGeneration, "strong-model"),
            (OperationType::EditProposal, "strong-model"),
            (OperationType::StaleDetection, "fast-model"),
            (OperationType::Embedding, "embed-model"),
        ] {
            routes.insert(op, (mock.clone() as Arc<dyn LlmProvider>, model.to_owned()));
        }
        ModelRouter::from_routes(routes)
    }

    #[tokio::test]
    async fn router_dispatches_to_correct_model() {
        let mock = Arc::new(MockProvider::new("ok", &["hello", " world"]));
        let router = router_with_mock(mock);

        let doc = router
            .complete(OperationType::DocumentGeneration, "prompt")
            .await
            .unwrap();
        assert_eq!(doc.usage.model, "strong-model");

        let cls = router
            .complete(OperationType::RetrievalClassification, "prompt")
            .await
            .unwrap();
        assert_eq!(cls.usage.model, "fast-model");
    }

    #[tokio::test]
    async fn stream_yields_chunks_then_usage() {
        let mock = Arc::new(MockProvider::new("", &["foo", "bar", "baz"]));
        let router = router_with_mock(mock);

        let mut stream = router
            .stream(OperationType::DocumentGeneration, "prompt")
            .await
            .unwrap();

        let mut deltas = Vec::new();
        let mut saw_usage = false;
        while let Some(item) = stream.next().await {
            match item.unwrap() {
                StreamItem::Chunk(c) => deltas.push(c.delta),
                StreamItem::Usage(_) => saw_usage = true,
            }
        }

        assert_eq!(deltas, ["foo", "bar", "baz"]);
        assert!(saw_usage, "stream must end with a Usage item");
    }

    #[tokio::test]
    async fn stream_cancellation_does_not_panic() {
        let mock = Arc::new(MockProvider::new("", &["a", "b", "c", "d", "e"]));
        let router = router_with_mock(mock);

        let mut stream = router
            .stream(OperationType::DocumentGeneration, "prompt")
            .await
            .unwrap();

        // Consume only the first item, then drop.
        let _ = stream.next().await;
        drop(stream);
        // If the spawned task panics on a cancelled sender, this test would fail.
    }

    #[tokio::test]
    async fn meter_aggregates_token_counts() {
        let mock = Arc::new(MockProvider::with_usage("ok", 10, 20));
        let router = router_with_mock(mock);

        router
            .complete(OperationType::DocumentGeneration, "p1")
            .await
            .unwrap();
        router
            .complete(OperationType::EditProposal, "p2")
            .await
            .unwrap();
        router.embed("text").await.unwrap();

        assert_eq!(router.meter.total_input_tokens(), 30);
        assert_eq!(router.meter.total_output_tokens(), 40);
        assert_eq!(router.meter.snapshot().len(), 3);
    }

    #[test]
    fn config_missing_operation_fails() {
        let yaml = r#"
routing:
  document_generation:
    provider: openai
    model: gpt-4o
providers:
  openai:
    api_key_env: OPENAI_API_KEY
"#;
        let err = config::RoutingConfig::from_yaml_str(yaml).unwrap_err();
        assert!(matches!(err, ConfigError::MissingRoute(_)));
    }

    #[test]
    fn config_unknown_provider_fails() {
        let yaml = r#"
routing:
  retrieval_classification:
    provider: nonexistent
    model: gpt-4o-mini
  document_generation:
    provider: nonexistent
    model: gpt-4o
  edit_proposal:
    provider: nonexistent
    model: gpt-4o
  stale_detection:
    provider: nonexistent
    model: gpt-4o-mini
  embedding:
    provider: nonexistent
    model: text-embedding-3-small
providers:
  openai:
    api_key_env: OPENAI_API_KEY
"#;
        let err = config::RoutingConfig::from_yaml_str(yaml).unwrap_err();
        assert!(matches!(err, ConfigError::UnknownProvider(_)));
    }
}
