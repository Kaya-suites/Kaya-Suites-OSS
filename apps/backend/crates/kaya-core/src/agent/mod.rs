//! Agent loop — plan, dispatch tools, stream events (FR-14, FR-15).
//!
//! # Propose-then-approve invariant (FR-15)
//!
//! Tools that mutate storage (`create_document`, `propose_edit`) produce a
//! [`ProposedEdit`] wrapped in [`AgentEvent::ProposedEditEmitted`]. The edit is
//! *not* applied to storage until the caller obtains an [`ApprovalToken`] via
//! [`crate::auth::UserSession::approve_edit`] and calls
//! [`crate::edit::commit_edit`].
//!
//! # Usage
//!
//! ```no_run
//! use std::sync::Arc;
//! use kaya_core::agent::{AgentContext, AgentEvent, AgentLoop, InvocationLog};
//! use kaya_core::agent::tools::default_tools;
//! use futures::StreamExt;
//!
//! # async fn example(ctx: Arc<AgentContext>) {
//! let log = Arc::new(InvocationLog::new());
//! let agent = AgentLoop::new(default_tools());
//! let mut stream = agent.run("Update the onboarding doc".into(), vec![], ctx, log.clone());
//!
//! while let Some(event) = stream.next().await {
//!     match event.unwrap() {
//!         AgentEvent::FinalMessage { text } => println!("{text}"),
//!         AgentEvent::ProposedEditEmitted { edit } => { /* present diff to user */ let _ = edit; }
//!         _ => {}
//!     }
//! }
//! # }
//! ```

pub mod log;
pub mod tool;
pub mod tools;

pub use log::{InvocationLog, ToolInvocation};
pub use tool::{Tool, ToolOutput};
pub use tools::default_tools;

use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use futures::channel::mpsc;
use futures::stream::BoxStream;
use futures::SinkExt;
use uuid::Uuid;

use crate::auth::UserSession;
use crate::edit::ProposedEdit;
use crate::error::KayaError;
use crate::model_router::{ModelRouter, OperationType, ToolCallRequest, ToolDefinition};
use crate::storage::StorageAdapter;

// ── Context ───────────────────────────────────────────────────────────────────

/// Shared context threaded through every tool invocation.
pub struct AgentContext {
    pub storage: Arc<dyn StorageAdapter>,
    pub router: Arc<ModelRouter>,
    pub session: UserSession,
}

// ── Events ────────────────────────────────────────────────────────────────────

/// An event emitted by the agent loop stream.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// Incremental reasoning text (emitted if the model streams thinking).
    ThinkingChunk { text: String },
    /// The model decided to call a tool.
    ToolCall { name: String, input: serde_json::Value },
    /// A tool returned a result (or error).
    ToolResult {
        name: String,
        output: serde_json::Value,
        latency_ms: u64,
    },
    /// A tool produced a pending edit. The edit is *not* applied to storage
    /// until the caller approves it via [`crate::edit::commit_edit`].
    ProposedEditEmitted { edit: ProposedEdit },
    /// The model's final text response — the agent turn is complete.
    FinalMessage { text: String },
}

// ── Loop ─────────────────────────────────────────────────────────────────────

/// Drives the agent planning-and-tool-dispatch loop.
///
/// Construct once with [`AgentLoop::new`]; call [`AgentLoop::run`] for each
/// user turn. The loop is stateless between turns — the caller manages session
/// history if continuity is needed.
pub struct AgentLoop {
    tools: Vec<Arc<dyn Tool>>,
    /// Hard cap on tool-call iterations per turn to prevent infinite loops.
    max_turns: usize,
}

impl AgentLoop {
    pub fn new(tools: Vec<Arc<dyn Tool>>) -> Self {
        Self { tools, max_turns: 10 }
    }

    pub fn with_max_turns(mut self, n: usize) -> Self {
        self.max_turns = n;
        self
    }

    /// Run a single agent turn for `message`.
    ///
    /// `prior_turns` is the conversation history as `(role, content)` pairs in
    /// chronological order. Pass an empty slice for a fresh session.
    ///
    /// Returns a stream of [`AgentEvent`]s. The stream ends after
    /// [`AgentEvent::FinalMessage`]. Dropping the stream mid-turn cancels
    /// cleanly (the spawned task notices the closed sender and returns).
    pub fn run(
        &self,
        message: String,
        prior_turns: Vec<(String, String)>,
        ctx: Arc<AgentContext>,
        log: Arc<InvocationLog>,
    ) -> BoxStream<'static, Result<AgentEvent, KayaError>> {
        let tools = self.tools.clone();
        let max_turns = self.max_turns;

        let (tx, rx) = mpsc::channel::<Result<AgentEvent, KayaError>>(32);

        tokio::spawn(async move {
            agent_task(message, prior_turns, ctx, log, tools, max_turns, tx).await;
        });

        Box::pin(rx)
    }
}

// ── Internal task ─────────────────────────────────────────────────────────────

async fn agent_task(
    message: String,
    prior_turns: Vec<(String, String)>,
    ctx: Arc<AgentContext>,
    log: Arc<InvocationLog>,
    tools: Vec<Arc<dyn Tool>>,
    max_turns: usize,
    mut tx: mpsc::Sender<Result<AgentEvent, KayaError>>,
) {
    let tool_defs: Vec<ToolDefinition> = tools
        .iter()
        .map(|t| ToolDefinition {
            name: t.name().to_owned(),
            description: t.description().to_owned(),
            parameters: t.schema(),
        })
        .collect();

    let system_prompt = build_system_prompt(&tools);
    let turn_id = Uuid::new_v4();
    let mut tool_history = String::new();

    // Format prior conversation turns as context before the current message.
    let conversation_context = if prior_turns.is_empty() {
        String::new()
    } else {
        let turns = prior_turns
            .iter()
            .map(|(role, content)| {
                let label = if role == "assistant" { "Assistant" } else { "User" };
                format!("{label}: {content}")
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        format!("\n\nConversation history:\n{turns}\n")
    };

    for _ in 0..max_turns {
        let prompt = format!("{system_prompt}{conversation_context}\nUser: {message}\n{tool_history}");

        let req = ToolCallRequest {
            prompt,
            model: String::new(), // ModelRouter fills this from the routing table
            operation: OperationType::EditProposal,
            tools: tool_defs.clone(),
        };

        let resp = match ctx.router.tool_call(OperationType::EditProposal, req).await {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(Err(e)).await;
                return;
            }
        };

        match resp.result {
            Some(tool_call) => {
                // ── Emit ToolCall event ──────────────────────────────────────
                if tx
                    .send(Ok(AgentEvent::ToolCall {
                        name: tool_call.tool_name.clone(),
                        input: tool_call.arguments.clone(),
                    }))
                    .await
                    .is_err()
                {
                    return; // stream was dropped (cancelled)
                }

                // ── Invoke the tool ──────────────────────────────────────────
                let started_at = Utc::now();
                let t0 = Instant::now();

                let (output_json, maybe_edit, error_str) =
                    match tools.iter().find(|t| t.name() == tool_call.tool_name) {
                        None => {
                            let e = format!("Unknown tool: {}", tool_call.tool_name);
                            (serde_json::json!({ "error": &e }), None, Some(e))
                        }
                        Some(t) => match t.invoke(tool_call.arguments.clone(), &ctx).await {
                            Ok(out) => (out.content, out.proposed_edit, None),
                            Err(e) => {
                                let s = e.to_string();
                                (serde_json::json!({ "error": &s }), None, Some(s))
                            }
                        },
                    };

                let latency_ms = t0.elapsed().as_millis() as u64;

                // ── Record invocation ────────────────────────────────────────
                log.record(ToolInvocation {
                    id: Uuid::new_v4(),
                    turn_id,
                    tool_name: tool_call.tool_name.clone(),
                    input: tool_call.arguments.clone(),
                    output: error_str
                        .as_ref()
                        .map(|e| Err(e.clone()))
                        .unwrap_or_else(|| Ok(output_json.clone())),
                    latency_ms,
                    started_at,
                });

                // ── Emit ToolResult ──────────────────────────────────────────
                if tx
                    .send(Ok(AgentEvent::ToolResult {
                        name: tool_call.tool_name.clone(),
                        output: output_json.clone(),
                        latency_ms,
                    }))
                    .await
                    .is_err()
                {
                    return;
                }

                // ── Emit ProposedEditEmitted if the tool produced one ─────────
                if let Some(edit) = maybe_edit {
                    if tx
                        .send(Ok(AgentEvent::ProposedEditEmitted { edit }))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }

                // ── Append to within-turn tool history ──────────────────────
                let result_json = serde_json::to_string(&output_json).unwrap_or_default();
                let args_json =
                    serde_json::to_string(&tool_call.arguments).unwrap_or_default();
                tool_history.push_str(&format!(
                    "\n[Calling: {}({args_json})]\n[Result]: {result_json}\n",
                    tool_call.tool_name,
                ));
            }

            None => {
                // ── Model is done — emit final message ───────────────────────
                let text = resp
                    .content
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "Done.".to_owned());
                let _ = tx.send(Ok(AgentEvent::FinalMessage { text })).await;
                return;
            }
        }
    }

    // Exceeded max_turns — still emit a final message so the stream closes.
    let _ = tx
        .send(Ok(AgentEvent::FinalMessage {
            text: "Reached maximum agent turns.".to_owned(),
        }))
        .await;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn build_system_prompt(tools: &[Arc<dyn Tool>]) -> String {
    let tool_list = tools
        .iter()
        .map(|t| format!("- {}: {}", t.name(), t.description()))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "You are a knowledge management assistant for Kaya Suites.\n\
         You help users search, read, and update their document knowledge base.\n\
         \n\
         Available tools:\n\
         {tool_list}\n\
         \n\
         When you need information or want to make a change, call the appropriate \
         tool. The results will be shown to you. Once you have gathered enough \
         information, provide a final answer to the user without calling any more \
         tools.\n\
         \n\
         CITATION RULES (required):\n\
         When your final answer references specific content from a document, you \
         MUST embed a citation marker immediately after the cited sentence using \
         this exact format: [[DOC_ID:PARA_ID]]\n\
         Replace DOC_ID with the document's UUID and PARA_ID with the paragraph_id \
         from the tool result. These markers are rendered as clickable citation chips \
         in the UI.\n\
         Example: \"Revenue grew 12% last quarter. [[d1a2b3c4-e5f6-7890-abcd-ef1234567890:abc123def456]]\"\n\
         Always include these markers when you have retrieved and used document content.\n\
         \n\
         IMPORTANT: Never apply document edits directly. Always use propose_edit \
         or create_document so the user can review and approve the change."
    )
}
