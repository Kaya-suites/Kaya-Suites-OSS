//! Thin `reqwest`-based Anthropic Messages API client.
//!
//! **This is the only file in `kaya-core` that may reference Anthropic-specific
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

const BASE_URL: &str = "https://api.anthropic.com/v1";
const API_VERSION: &str = "2023-06-01";
const DEFAULT_MAX_TOKENS: u32 = 4096;

pub struct AnthropicProvider {
    client: Client,
    api_key: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String) -> Self {
        Self { client: Client::new(), api_key }
    }

    fn request(&self, path: &str) -> reqwest::RequestBuilder {
        self.client
            .post(format!("{BASE_URL}{path}"))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
    }

    async fn check_status(resp: reqwest::Response) -> Result<reqwest::Response, KayaError> {
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let body = resp.text().await.unwrap_or_default();
        Err(KayaError::Internal(format!("Anthropic {status}: {body}")))
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, KayaError> {
        let body = json!({
            "model": request.model,
            "max_tokens": request.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
            "messages": [{"role": "user", "content": request.prompt}],
        });

        let resp = Self::check_status(
            self.request("/messages")
                .json(&body)
                .send()
                .await
                .map_err(|e| KayaError::Internal(e.to_string()))?,
        )
        .await?;

        let json: Value =
            resp.json().await.map_err(|e| KayaError::Internal(e.to_string()))?;

        let content = extract_text_content(&json);
        let (input_tokens, output_tokens) = extract_usage(&json);
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
            "messages": [{"role": "user", "content": request.prompt}],
        });

        let resp = Self::check_status(
            self.request("/messages")
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

    async fn embed(&self, _request: EmbeddingRequest) -> Result<EmbeddingResponse, KayaError> {
        Err(KayaError::Internal(
            "Anthropic does not provide an embeddings endpoint; \
             route OperationType::Embedding to OpenAI"
                .to_owned(),
        ))
    }

    async fn tool_call(&self, request: ToolCallRequest) -> Result<ToolCallResponse, KayaError> {
        let tools: Vec<Value> = request
            .tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters,
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
            self.request("/messages")
                .json(&body)
                .send()
                .await
                .map_err(|e| KayaError::Internal(e.to_string()))?,
        )
        .await?;

        let json: Value =
            resp.json().await.map_err(|e| KayaError::Internal(e.to_string()))?;

        let blocks = json["content"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);

        let tool_result = blocks
            .iter()
            .find(|b| b["type"].as_str() == Some("tool_use"))
            .map(|b| ToolCallResult {
                tool_name: b["name"].as_str().unwrap_or("").to_owned(),
                arguments: b["input"].clone(),
            });

        // Capture text response when the model chose not to call a tool.
        let text_content = if tool_result.is_none() {
            blocks
                .iter()
                .find(|b| b["type"].as_str() == Some("text"))
                .and_then(|b| b["text"].as_str())
                .map(str::to_owned)
        } else {
            None
        };

        let (input_tokens, output_tokens) = extract_usage(&json);
        let model = json["model"].as_str().unwrap_or(&request.model).to_owned();

        Ok(ToolCallResponse {
            result: tool_result,
            content: text_content,
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

/// Consume the bytes stream from a streaming Anthropic response, parse SSE
/// events, and forward [`StreamItem`]s to `tx`.
///
/// Anthropic SSE event types we care about:
/// - `message_start`     → captures `message.usage.input_tokens`
/// - `content_block_delta` (text_delta) → emits `StreamItem::Chunk`
/// - `message_delta`     → emits `StreamItem::Usage` with final output_tokens
/// - `message_stop`      → ends the stream
async fn drive_sse(
    bytes_stream: impl futures::Stream<Item = Result<Bytes, reqwest::Error>> + Send,
    model: String,
    operation: OperationType,
    mut tx: mpsc::Sender<Result<StreamItem, KayaError>>,
) {
    let mut buf = String::new();
    let mut input_tokens: u32 = 0;

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

        // Process all complete lines currently in the buffer.
        loop {
            match buf.find('\n') {
                None => break,
                Some(pos) => {
                    let line = buf[..pos].trim_end_matches('\r').to_owned();
                    buf = buf[pos + 1..].to_owned();

                    let Some(data) = line.strip_prefix("data: ") else {
                        continue;
                    };

                    let Ok(event) = serde_json::from_str::<Value>(data) else {
                        continue;
                    };

                    match event["type"].as_str() {
                        Some("message_start") => {
                            input_tokens = event["message"]["usage"]["input_tokens"]
                                .as_u64()
                                .unwrap_or(0) as u32;
                        }
                        Some("content_block_delta") => {
                            if event["delta"]["type"].as_str() == Some("text_delta") {
                                if let Some(text) = event["delta"]["text"].as_str() {
                                    if tx
                                        .send(Ok(StreamItem::Chunk(StreamChunk {
                                            delta: text.to_owned(),
                                        })))
                                        .await
                                        .is_err()
                                    {
                                        return; // receiver dropped (cancelled)
                                    }
                                }
                            }
                        }
                        Some("message_delta") => {
                            let output_tokens =
                                event["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32;
                            // Best-effort: if the receiver is already gone, just stop.
                            let _ = tx
                                .send(Ok(StreamItem::Usage(TokenUsage {
                                    input_tokens,
                                    output_tokens,
                                    model: model.clone(),
                                    operation: operation.clone(),
                                })))
                                .await;
                        }
                        Some("message_stop") => return,
                        _ => {}
                    }
                }
            }
        }
    }
}

// ---- Helpers --------------------------------------------------------------

fn extract_text_content(json: &Value) -> String {
    json["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|block| {
            if block["type"].as_str() == Some("text") {
                block["text"].as_str().map(str::to_owned)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("")
}

fn extract_usage(json: &Value) -> (u32, u32) {
    let input = json["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32;
    let output = json["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32;
    (input, output)
}
