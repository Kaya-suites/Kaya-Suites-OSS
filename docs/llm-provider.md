# LLM Provider

**Trait location:** `crates/kaya-core/src/model_router/`  
**License:** Apache 2.0

## Design rule

**No code outside a provider implementation file (`anthropic.rs`, `openai.rs`) may import a vendor SDK.** All LLM calls in business logic must go through `LlmProvider` or `ModelRouter`.

## Operation types

`OperationType` maps a logical operation to a `(provider, model)` pair via the routing config:

| Variant | Default model | Purpose |
|---|---|---|
| `RetrievalClassification` | `gpt-4o-mini` | Classify retrieved documents for relevance |
| `DocumentGeneration` | `claude-opus-4-6` | Generate new document content |
| `EditProposal` | `claude-opus-4-6` | Propose an edit to an existing document |
| `StaleDetection` | `gpt-4o-mini` | Detect whether a document is stale |
| `Embedding` | `text-embedding-3-small` | Embed text for vector search |

## Trait methods

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, KayaError>;
    async fn stream(&self, request: CompletionRequest) -> Result<BoxStream<'static, Result<StreamItem, KayaError>>, KayaError>;
    async fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, KayaError>;
    async fn tool_call(&self, request: ToolCallRequest) -> Result<ToolCallResponse, KayaError>;
}
```

### `stream` contract

The stream yields `StreamItem::Chunk` items followed by exactly one `StreamItem::Usage` as the final item. Dropping the stream before exhaustion cancels it cleanly — no panic.

### `tool_call` response

`ToolCallResponse.result` is `Some` when the model chose a tool. `ToolCallResponse.content` is `Some` when the model chose to reply in text instead. Exactly one of the two is set for a successful call.

## `ModelRouter`

`ModelRouter` dispatches each operation to the correct provider/model pair and accumulates token usage via its embedded `Meter`.

```rust
// Business logic calls the router, not a provider directly:
let response = router.complete(OperationType::EditProposal, "prompt text").await?;
let stream   = router.stream(OperationType::DocumentGeneration, "prompt").await?;
let vector   = router.embed("paragraph text").await?;
```

### Metering

```rust
router.meter.total_input_tokens()   // aggregated across all calls this session
router.meter.total_output_tokens()
router.meter.snapshot()             // per-operation breakdown
```

## Routing configuration (`kaya.yaml`)

```yaml
routing:
  retrieval_classification:
    provider: openai
    model: gpt-4o-mini
  document_generation:
    provider: anthropic
    model: claude-opus-4-6
  edit_proposal:
    provider: anthropic
    model: claude-opus-4-6
  stale_detection:
    provider: openai
    model: gpt-4o-mini
  embedding:
    provider: openai
    model: text-embedding-3-small

providers:
  openai:
    api_key_env: OPENAI_API_KEY
  anthropic:
    api_key_env: ANTHROPIC_API_KEY
```

All five operation types must be present; missing entries and references to unknown providers are caught at startup with a descriptive `ConfigError`.

## Adding a new provider

1. Create `crates/kaya-core/src/model_router/<vendor>.rs`.
2. Import the vendor SDK **only** in that file.
3. Implement `LlmProvider` for the new struct.
4. Register the provider name in `config.rs`.
5. Add the provider block to `kaya.yaml`.
