//! LlmProvider trait — abstracts over LLM vendor SDKs.
//!
//! No code outside of a provider implementation file may import a vendor SDK
//! directly. All LLM calls go through this trait.

use async_trait::async_trait;
use futures::stream::BoxStream;
use crate::error::KayaError;

/// A structured tool/function definition passed to the model.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    /// JSON Schema describing the tool's parameters.
    pub parameters: serde_json::Value,
}

/// The result of a model tool/function call.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCallResult {
    pub tool_name: String,
    /// Raw JSON arguments chosen by the model.
    pub arguments: serde_json::Value,
}

/// Abstracts over LLM providers (OpenAI, Anthropic, local, etc.).
///
/// No vendor SDK may be imported outside of the concrete implementation of
/// this trait. All LLM calls in business logic must go through `LlmProvider`.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Send a single-turn completion request and return the full response text.
    async fn complete(&self, prompt: &str) -> Result<String, KayaError>;

    /// Stream a completion, yielding text chunks as they arrive.
    async fn stream<'a>(
        &'a self,
        prompt: &'a str,
    ) -> Result<BoxStream<'a, Result<String, KayaError>>, KayaError>;

    /// Generate a vector embedding for `text`.
    async fn embed(&self, text: &str) -> Result<Vec<f32>, KayaError>;

    /// Send a completion request with tool definitions and return the model's
    /// chosen tool call (if any).
    async fn tool_call(
        &self,
        prompt: &str,
        tools: &[ToolDefinition],
    ) -> Result<Option<ToolCallResult>, KayaError>;
}
