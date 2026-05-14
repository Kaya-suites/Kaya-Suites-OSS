//! [`ModelRouter`] — the public entry point for all LLM calls.
//!
//! Loads the routing table from `kaya.yaml`, instantiates providers, and
//! dispatches every call to the right `(provider, model)` pair. All token
//! usage is aggregated in the embedded [`Meter`].

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use futures::stream::BoxStream;

use crate::error::KayaError;

use super::anthropic::AnthropicProvider;
use super::config::{ConfigError, RoutingConfig};
use super::meter::Meter;
use super::openai::OpenAIProvider;
use super::{
    CompletionRequest, CompletionResponse, EmbeddingRequest, EmbeddingResponse, LlmProvider,
    OperationType, StreamItem, ToolCallRequest, ToolCallResponse,
};

struct Route {
    provider: Arc<dyn LlmProvider>,
    model: String,
}

/// Routes LLM operations to the configured provider + model and aggregates
/// token usage in the embedded [`Meter`].
pub struct ModelRouter {
    routes: HashMap<OperationType, Route>,
    /// Token-usage aggregator.
    pub meter: Arc<Meter>,
}

impl ModelRouter {
    /// Build from a parsed, validated [`RoutingConfig`].
    ///
    /// Reads API keys from the environment variables declared in the config.
    pub fn from_config(config: &RoutingConfig) -> Result<Self, ConfigError> {
        let mut providers: HashMap<String, Arc<dyn LlmProvider>> = HashMap::new();

        for name in config.providers.keys() {
            let api_key = config.resolve_api_key(name)?;
            let provider: Arc<dyn LlmProvider> = match name.as_str() {
                "anthropic" => Arc::new(AnthropicProvider::new(api_key)),
                "openai" => Arc::new(OpenAIProvider::new(api_key)),
                other => return Err(ConfigError::UnknownProvider(other.to_owned())),
            };
            providers.insert(name.clone(), provider);
        }

        let mut routes = HashMap::new();
        for (op, entry) in &config.routing {
            let provider = providers[&entry.provider].clone();
            routes.insert(
                op.clone(),
                Route { provider, model: entry.model.clone() },
            );
        }

        Ok(Self { routes, meter: Arc::new(Meter::new()) })
    }

    /// Load `kaya.yaml` from `path` and build the router.
    pub fn from_yaml(path: &Path) -> Result<Self, ConfigError> {
        let config = RoutingConfig::from_yaml_file(path)?;
        Self::from_config(&config)
    }

    /// Construct from a pre-built routing map. Intended for testing and DI.
    pub fn from_routes(routes: HashMap<OperationType, (Arc<dyn LlmProvider>, String)>) -> Self {
        let routes = routes
            .into_iter()
            .map(|(op, (provider, model))| (op, Route { provider, model }))
            .collect();
        Self { routes, meter: Arc::new(Meter::new()) }
    }

    fn route(&self, op: &OperationType) -> Result<(&dyn LlmProvider, &str), KayaError> {
        self.routes
            .get(op)
            .map(|r| (r.provider.as_ref(), r.model.as_str()))
            .ok_or_else(|| KayaError::Internal(format!("no route configured for {op:?}")))
    }

    pub async fn complete(
        &self,
        op: OperationType,
        prompt: impl Into<String>,
    ) -> Result<CompletionResponse, KayaError> {
        let (provider, model) = self.route(&op)?;
        let req = CompletionRequest {
            prompt: prompt.into(),
            model: model.to_owned(),
            operation: op,
            max_tokens: None,
        };
        let resp = provider.complete(req).await?;
        self.meter.record(resp.usage.clone());
        Ok(resp)
    }

    pub async fn stream(
        &self,
        op: OperationType,
        prompt: impl Into<String>,
    ) -> Result<BoxStream<'static, Result<StreamItem, KayaError>>, KayaError> {
        let (provider, model) = self.route(&op)?;
        let req = CompletionRequest {
            prompt: prompt.into(),
            model: model.to_owned(),
            operation: op,
            max_tokens: None,
        };
        // Note: streaming token usage is reported via StreamItem::Usage inside
        // the stream; the router does NOT record it to the meter here (the
        // caller owns the stream and decides when/whether to consume it).
        provider.stream(req).await
    }

    pub async fn embed(&self, text: impl Into<String>) -> Result<EmbeddingResponse, KayaError> {
        let (provider, model) = self.route(&OperationType::Embedding)?;
        let req = EmbeddingRequest { text: text.into(), model: model.to_owned() };
        let resp = provider.embed(req).await?;
        self.meter.record(resp.usage.clone());
        Ok(resp)
    }

    pub async fn tool_call(
        &self,
        op: OperationType,
        request: ToolCallRequest,
    ) -> Result<ToolCallResponse, KayaError> {
        let (provider, model) = self.route(&op)?;
        let req = ToolCallRequest { model: model.to_owned(), ..request };
        let resp = provider.tool_call(req).await?;
        self.meter.record(resp.usage.clone());
        Ok(resp)
    }
}
