use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;

use axum::{
    Json,
    body::Body,
    extract::{Extension, Path},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::Mutex;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use kaya_core::{
    ParagraphChange, ProposedEdit, ProposedEditKind, SessionStorage, StorageAdapter,
    agent::{AgentContext, AgentEvent, AgentLoop, InvocationLog},
    agent::tools::default_tools,
    auth::UserSession,
    model_router::ModelRouter,
};

use crate::state::StoredEdit;

#[derive(Deserialize)]
pub struct ChatBody {
    pub message: String,
}

pub async fn chat_stream(
    Extension(storage): Extension<Arc<dyn StorageAdapter>>,
    Extension(sessions): Extension<Arc<dyn SessionStorage>>,
    Extension(llm): Extension<Option<Arc<ModelRouter>>>,
    Extension(pending_edits): Extension<Arc<Mutex<HashMap<Uuid, StoredEdit>>>>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<ChatBody>,
) -> Response {
    let Some(router) = llm else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "LLM provider not configured"})),
        )
            .into_response();
    };

    let prior_messages = sessions
        .get_prior_messages(session_id)
        .await
        .unwrap_or_default();

    let _ = sessions.touch_session(session_id).await;

    let _ = sessions
        .save_user_message(session_id, &Uuid::new_v4().to_string(), &body.message)
        .await;

    let (tx, rx) = tokio::sync::mpsc::channel::<Bytes>(64);
    let message = body.message;

    tokio::spawn(async move {
        run_agent_stream(
            storage,
            sessions,
            pending_edits,
            router,
            session_id,
            message,
            prior_messages,
            tx,
        )
        .await;
    });

    let stream = ReceiverStream::new(rx).map(Ok::<_, Infallible>);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .header("X-Accel-Buffering", "no")
        .body(Body::from_stream(stream))
        .unwrap()
}

async fn run_agent_stream(
    storage: Arc<dyn StorageAdapter>,
    sessions: Arc<dyn SessionStorage>,
    pending_edits: Arc<Mutex<HashMap<Uuid, StoredEdit>>>,
    router: Arc<ModelRouter>,
    session_id: Uuid,
    message: String,
    prior_messages: Vec<(String, String)>,
    tx: tokio::sync::mpsc::Sender<Bytes>,
) {
    let session = UserSession { user_id: Uuid::nil() };
    let ctx = Arc::new(AgentContext {
        storage: storage.clone(),
        router,
        session,
    });
    let log = Arc::new(InvocationLog::new());
    let agent = AgentLoop::new(default_tools());
    let mut events = agent.run(message, prior_messages, ctx, log);

    let mut doc_title_cache: HashMap<Uuid, String> = HashMap::new();
    let mut assistant_text = String::new();
    let mut assistant_citations: Vec<Value> = Vec::new();

    macro_rules! send {
        ($data:expr) => {{
            let line = format!("data: {}\n\n", $data);
            if tx.send(Bytes::from(line)).await.is_err() {
                return;
            }
        }};
    }

    while let Some(result) = events.next().await {
        match result {
            Err(e) => {
                send!(json!({"type": "Error", "message": e.to_string()}));
                break;
            }

            Ok(AgentEvent::ToolResult { name, output, .. }) => {
                match name.as_str() {
                    "search_documents" => {
                        if let Some(arr) = output.get("documents").and_then(|v| v.as_array()) {
                            for item in arr {
                                if let (Some(id_str), Some(title)) =
                                    (item["id"].as_str(), item["title"].as_str())
                                {
                                    if let Ok(id) = Uuid::parse_str(id_str) {
                                        doc_title_cache.insert(id, title.to_string());
                                    }
                                }
                            }
                        }
                    }
                    "read_document" => {
                        if let (Some(id_str), Some(title)) =
                            (output["id"].as_str(), output["title"].as_str())
                        {
                            if let Ok(id) = Uuid::parse_str(id_str) {
                                doc_title_cache.insert(id, title.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }

            Ok(AgentEvent::ProposedEditEmitted { edit }) => {
                if let Some(sse_data) =
                    build_edit_sse(&storage, &pending_edits, &edit).await
                {
                    send!(sse_data);
                }
            }

            Ok(AgentEvent::FinalMessage { text }) => {
                let (clean_text, raw_citations) = extract_citations(&text);

                for (label, (doc_id_str, para_id)) in raw_citations.iter().enumerate() {
                    let label = label + 1;
                    let doc_id = Uuid::parse_str(doc_id_str).unwrap_or(Uuid::nil());

                    let title = if let Some(t) = doc_title_cache.get(&doc_id) {
                        t.clone()
                    } else {
                        storage
                            .get_document(doc_id)
                            .await
                            .map(|d| d.title)
                            .unwrap_or_default()
                    };

                    assistant_citations.push(json!({
                        "label": label,
                        "docId": doc_id_str,
                        "paragraphId": para_id,
                        "title": title,
                    }));

                    send!(json!({
                        "type": "CitationFound",
                        "docId": doc_id_str,
                        "paragraphId": para_id,
                        "label": label,
                        "title": title,
                    }));
                }

                for chunk in clean_text
                    .as_bytes()
                    .chunks(80)
                    .map(|c| std::str::from_utf8(c).unwrap_or_default())
                {
                    send!(json!({"type": "TextChunk", "content": chunk}));
                    tokio::time::sleep(tokio::time::Duration::from_millis(15)).await;
                }

                assistant_text = clean_text;
            }

            Ok(_) => {}
        }
    }

    if !assistant_text.is_empty() {
        let citations_json =
            serde_json::to_string(&assistant_citations).unwrap_or_else(|_| "[]".to_string());
        let _ = sessions
            .save_assistant_message(
                session_id,
                &Uuid::new_v4().to_string(),
                &assistant_text,
                &citations_json,
            )
            .await;
    }

    send!(json!({"type": "Done"}));
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn extract_citations(text: &str) -> (String, Vec<(String, String)>) {
    let mut result = String::with_capacity(text.len());
    let mut citations: Vec<(String, String)> = Vec::new();
    let mut remaining = text;

    while let Some(start) = remaining.find("[[") {
        result.push_str(&remaining[..start]);
        remaining = &remaining[start + 2..];

        if let Some(end) = remaining.find("]]") {
            let inner = &remaining[..end];
            remaining = &remaining[end + 2..];

            if let Some(colon) = inner.find(':') {
                let doc_id = inner[..colon].trim().to_string();
                let para_id = inner[colon + 1..].trim().to_string();

                let label = citations
                    .iter()
                    .position(|(d, p)| d == &doc_id && p == &para_id)
                    .map(|i| i + 1)
                    .unwrap_or_else(|| {
                        citations.push((doc_id, para_id));
                        citations.len()
                    });

                result.push_str(&format!("[{label}]"));
            } else {
                result.push_str("[[");
                result.push_str(inner);
                result.push_str("]]");
            }
        } else {
            result.push_str("[[");
            result.push_str(remaining);
            remaining = "";
        }
    }

    result.push_str(remaining);
    (result, citations)
}

async fn build_edit_sse(
    storage: &Arc<dyn StorageAdapter>,
    pending_edits: &Arc<Mutex<HashMap<Uuid, StoredEdit>>>,
    edit: &ProposedEdit,
) -> Option<Value> {
    let (doc_id, para_id, original, proposed) = match &edit.kind {
        ProposedEditKind::Modify { document_id, diff, .. } => {
            let first = diff.changes.iter().find_map(|c| {
                if let ParagraphChange::Modify { paragraph_id, old_text, new_text } = c {
                    Some((paragraph_id.clone(), old_text.clone(), new_text.clone()))
                } else {
                    None
                }
            })?;
            (Some(*document_id), first.0, first.1, first.2)
        }
        ProposedEditKind::Create { title: _, body } => {
            (None, "p0".to_string(), String::new(), body.clone())
        }
        ProposedEditKind::UpdateContent { document_id, new_content } => {
            (Some(*document_id), "p0".to_string(), String::new(), new_content.clone())
        }
        ProposedEditKind::DeleteDocument { document_id } => {
            let doc_title = storage
                .get_document(*document_id)
                .await
                .map(|d| d.title)
                .unwrap_or_default();
            let stored = StoredEdit {
                edit: edit.clone(),
                doc_title: doc_title.clone(),
                first_paragraph_id: String::new(),
                original_paragraph: String::new(),
                proposed_paragraph: String::new(),
            };
            pending_edits.lock().await.insert(edit.id, stored);
            return Some(json!({
                "type": "ProposedDeleteEmitted",
                "editId": edit.id,
                "docId": document_id,
                "docTitle": doc_title,
            }));
        }
    };

    let doc_title = if let Some(id) = doc_id {
        storage.get_document(id).await.map(|d| d.title).unwrap_or_default()
    } else {
        String::new()
    };

    let stored = StoredEdit {
        edit: edit.clone(),
        doc_title,
        first_paragraph_id: para_id.clone(),
        original_paragraph: original.clone(),
        proposed_paragraph: proposed.clone(),
    };
    pending_edits.lock().await.insert(edit.id, stored);

    Some(json!({
        "type": "ProposedEditEmitted",
        "editId": edit.id,
        "docId": doc_id,
        "paragraphId": para_id,
        "original": original,
        "proposed": proposed,
    }))
}
