//! Thin `reqwest`-based OpenAI API client.
//!
//! **This is the only file in `kaya-core` that may reference OpenAI-specific
//! API shapes.** No vendor SDK is imported.

use async_trait::async_trait;
use bytes::Bytes;
use futures::channel::mpsc;
use futures::stream::BoxStream;
use futures::{SinkExt, StreamExt};
use reqwest::Client;
use serde_json::{json, Value};

use crate::error::KayaError;

use super::meter::TokenUsage;
use super::{
    CompletionRequest, CompletionResponse, EmbeddingRequest, EmbeddingResponse, LlmProvider,
    OperationType, StreamChunk, StreamItem, ToolCallRequest, ToolCallResponse, ToolCallResult,
};

const BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MAX_TOKENS: u32 = 4096;

pub struct OpenAIProvider {
    client: Client,
    api_key: String,
}

impl OpenAIProvider {
    pub fn new(api_key: String) -> Self {
        Self { client: Client::new(), api_key }
    }

    fn request(&self, path: &str) -> reqwest::RequestBuilder {
        self.client
            .post(format!("{BASE_URL}{path}"))
            .bearer_auth(&self.api_key)
    }

    async fn check_status(resp: reqwest::Response) -> Result<reqwest::Response, KayaError> {
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let body = resp.text().await.unwrap_or_default();
        Err(KayaError::Internal(format!("OpenAI {status}: {body}")))
    }
}

#[async_trait]
impl LlmProvider for OpenAIProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, KayaError> {
        let body = json!({
            "model": request.model,
            "max_tokens": request.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
            "messages": [{"role": "user", "content": request.prompt}],
        });

        let resp = Self::check_status(
            self.request("/chat/completions")
                .json(&body)
                .send()
                .await
                .map_err(|e| KayaError::Internal(e.to_string()))?,
        )
        .await?;

        let json: Value =
            resp.json().await.map_err(|e| KayaError::Internal(e.to_string()))?;

        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_owned();

        let input_tokens = json["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32;
        let output_tokens = json["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32;
        let model = json["model"].as_str().unwrap_or(&request.model).to_owned();

        Ok(CompletionResponse {
            content,
            usage: TokenUsage {
                input_tokens,
                output_tokens,
                model,
                operation: request.operation,
            },
        })
    }

    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> Result<BoxStream<'static, Result<StreamItem, KayaError>>, KayaError> {
        let body = json!({
            "model": request.model,
            "max_tokens": request.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
            "stream": true,
            "stream_options": {"include_usage": true},
            "messages": [{"role": "user", "content": request.prompt}],
        });

        let resp = Self::check_status(
            self.request("/chat/completions")
                .json(&body)
                .send()
                .await
                .map_err(|e| KayaError::Internal(e.to_string()))?,
        )
        .await?;

        let (tx, rx) = mpsc::channel::<Result<StreamItem, KayaError>>(32);
        let model = request.model.clone();
        let operation = request.operation.clone();
        let bytes_stream = resp.bytes_stream();

        tokio::spawn(async move {
            drive_sse(bytes_stream, model, operation, tx).await;
        });

        Ok(Box::pin(rx))
    }

    async fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, KayaError> {
        let body = json!({
            "model": request.model,
            "input": request.text,
        });

        let resp = Self::check_status(
            self.request("/embeddings")
                .json(&body)
                .send()
                .await
                .map_err(|e| KayaError::Internal(e.to_string()))?,
        )
        .await?;

        let json: Value =
            resp.json().await.map_err(|e| KayaError::Internal(e.to_string()))?;

        let embedding: Vec<f32> = json["data"][0]["embedding"]
            .as_array()
            .ok_or_else(|| KayaError::Internal("missing embedding in response".to_owned()))?
            .iter()
            .filter_map(|v| v.as_f64().map(|f| f as f32))
            .collect();

        let input_tokens = json["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32;
        let model = json["model"].as_str().unwrap_or(&request.model).to_owned();

        Ok(EmbeddingResponse {
            embedding,
            usage: TokenUsage {
                input_tokens,
                output_tokens: 0,
                model,
                operation: OperationType::Embedding,
            },
        })
    }

    async fn tool_call(&self, request: ToolCallRequest) -> Result<ToolCallResponse, KayaError> {
        let tools: Vec<Value> = request
            .tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    },
                })
            })
            .collect();

        let body = json!({
            "model": request.model,
            "max_tokens": DEFAULT_MAX_TOKENS,
            "tools": tools,
            "messages": [{"role": "user", "content": request.prompt}],
        });

        let resp = Self::check_status(
            self.request("/chat/completions")
                .json(&body)
                .send()
                .await
                .map_err(|e| KayaError::Internal(e.to_string()))?,
        )
        .await?;

        let json: Value =
            resp.json().await.map_err(|e| KayaError::Internal(e.to_string()))?;

        let tool_result = json["choices"][0]["message"]["tool_calls"][0]
            .as_object()
            .and_then(|tc| {
                let name = tc["function"]["name"].as_str()?.to_owned();
                let args_str = tc["function"]["arguments"].as_str()?;
                let arguments: Value = serde_json::from_str(args_str).ok()?;
                Some(ToolCallResult { tool_name: name, arguments })
            });

        let input_tokens = json["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32;
        let output_tokens = json["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32;
        let model = json["model"].as_str().unwrap_or(&request.model).to_owned();

        Ok(ToolCallResponse {
            result: tool_result,
            usage: TokenUsage {
                input_tokens,
                output_tokens,
                model,
                operation: request.operation,
            },
        })
    }
}

// ---- SSE driver -----------------------------------------------------------

/// Consume the bytes stream from a streaming OpenAI response, parse SSE
/// events, and forward [`StreamItem`]s to `tx`.
///
/// OpenAI SSE format (with `stream_options.include_usage = true`):
/// - Regular chunks: `choices[0].delta.content` contains text.
/// - Finish chunk: `choices[0].finish_reason = "stop"`, content is absent.
/// - Usage chunk: `usage` is populated, `choices` may be empty.
/// - Terminator: `data: [DONE]`
async fn drive_sse(
    bytes_stream: impl futures::Stream<Item = Result<Bytes, reqwest::Error>> + Send,
    model: String,
    operation: OperationType,
    mut tx: mpsc::Sender<Result<StreamItem, KayaError>>,
) {
    let mut buf = String::new();
    let mut input_tokens: u32 = 0;
    let mut output_tokens: u32 = 0;

    let mut stream = std::pin::pin!(bytes_stream);

    while let Some(chunk) = stream.next().await {
        let bytes = match chunk {
            Ok(b) => b,
            Err(e) => {
                let _ = tx.send(Err(KayaError::Internal(e.to_string()))).await;
                return;
            }
        };
        let text = match std::str::from_utf8(&bytes) {
            Ok(s) => s.to_owned(),
            Err(e) => {
                let _ = tx
                    .send(Err(KayaError::Internal(format!("UTF-8 decode: {e}"))))
                    .await;
                return;
            }
        };
        buf.push_str(&text);

        loop {
            match buf.find('\n') {
                None => break,
                Some(pos) => {
                    let line = buf[..pos].trim_end_matches('\r').to_owned();
                    buf = buf[pos + 1..].to_owned();

                    let Some(data) = line.strip_prefix("data: ") else {
                        continue;
                    };

                    if data == "[DONE]" {
                        // Emit accumulated usage and finish.
                        let _ = tx
                            .send(Ok(StreamItem::Usage(TokenUsage {
                                input_tokens,
                                output_tokens,
                                model: model.clone(),
                                operation: operation.clone(),
                            })))
                            .await;
                        return;
                    }

                    let Ok(event) = serde_json::from_str::<Value>(data) else {
                        continue;
                    };

                    // Collect usage whenever it appears.
                    if !event["usage"].is_null() {
                        input_tokens = event["usage"]["prompt_tokens"]
                            .as_u64()
                            .unwrap_or(input_tokens as u64) as u32;
                        output_tokens = event["usage"]["completion_tokens"]
                            .as_u64()
                            .unwrap_or(output_tokens as u64) as u32;
                    }

                    // Emit content delta if present.
                    if let Some(content) =
                        event["choices"][0]["delta"]["content"].as_str()
                    {
                        if !content.is_empty()
                            && tx
                                .send(Ok(StreamItem::Chunk(StreamChunk {
                                    delta: content.to_owned(),
                                })))
                                .await
                                .is_err()
                        {
                            return; // receiver dropped (cancelled)
                        }
                    }
                }
            }
        }
    }

    // Stream ended without [DONE] — still emit whatever usage we have.
    let _ = tx
        .send(Ok(StreamItem::Usage(TokenUsage {
            input_tokens,
            output_tokens,
            model,
            operation,
        })))
        .await;
}
