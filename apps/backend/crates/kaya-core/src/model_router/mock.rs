//! Mock LLM provider for unit tests.
//!
//! Returns deterministic canned responses; no network calls.

use async_trait::async_trait;
use futures::stream::BoxStream;

use crate::error::KayaError;

use super::meter::TokenUsage;
use super::{
    CompletionRequest, CompletionResponse, EmbeddingRequest, EmbeddingResponse, LlmProvider,
    OperationType, StreamChunk, StreamItem, ToolCallRequest, ToolCallResponse,
};

pub struct MockProvider {
    completion_text: String,
    stream_chunks: Vec<String>,
    embedding: Vec<f32>,
    input_tokens: u32,
    output_tokens: u32,
}

impl MockProvider {
    /// Default constructor: 10 input / 20 output tokens.
    pub fn new(completion_text: impl Into<String>, stream_chunks: &[&str]) -> Self {
        Self {
            completion_text: completion_text.into(),
            stream_chunks: stream_chunks.iter().map(|s| s.to_string()).collect(),
            embedding: vec![0.1, 0.2, 0.3],
            input_tokens: 10,
            output_tokens: 20,
        }
    }

    /// Constructor with explicit per-call token counts.
    pub fn with_usage(
        completion_text: impl Into<String>,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Self {
        Self {
            completion_text: completion_text.into(),
            stream_chunks: Vec::new(),
            embedding: vec![0.1, 0.2, 0.3],
            input_tokens,
            output_tokens,
        }
    }
}

#[async_trait]
impl LlmProvider for MockProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, KayaError> {
        Ok(CompletionResponse {
            content: self.completion_text.clone(),
            usage: TokenUsage {
                input_tokens: self.input_tokens,
                output_tokens: self.output_tokens,
                model: request.model,
                operation: request.operation,
            },
        })
    }

    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> Result<BoxStream<'static, Result<StreamItem, KayaError>>, KayaError> {
        let mut items: Vec<Result<StreamItem, KayaError>> = self
            .stream_chunks
            .iter()
            .map(|s| {
                Ok(StreamItem::Chunk(StreamChunk { delta: s.clone() }))
            })
            .collect();

        items.push(Ok(StreamItem::Usage(TokenUsage {
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            model: request.model,
            operation: request.operation,
        })));

        Ok(Box::pin(futures::stream::iter(items)))
    }

    async fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, KayaError> {
        Ok(EmbeddingResponse {
            embedding: self.embedding.clone(),
            usage: TokenUsage {
                input_tokens: self.input_tokens,
                output_tokens: 0,
                model: request.model,
                operation: OperationType::Embedding,
            },
        })
    }

    async fn tool_call(&self, request: ToolCallRequest) -> Result<ToolCallResponse, KayaError> {
        Ok(ToolCallResponse {
            result: None,
            usage: TokenUsage {
                input_tokens: self.input_tokens,
                output_tokens: self.output_tokens,
                model: request.model,
                operation: request.operation,
            },
        })
    }
}
