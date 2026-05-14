use std::sync::Arc;

use axum::{
    Json,
    body::Body,
    extract::{Extension, Path},
    http::{StatusCode, header},
    response::Response,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use kaya_core::{StorageAdapter, model_router::ModelRouter, retrieval::index_document_chunks};

use crate::error::ApiError;

// ── GET /documents ────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentSummary {
    id: Uuid,
    title: String,
    tags: Vec<String>,
    last_reviewed: Option<String>,
}

pub async fn list_documents(
    Extension(storage): Extension<Arc<dyn StorageAdapter>>,
) -> Result<Json<Vec<DocumentSummary>>, ApiError> {
    let docs = storage
        .list_documents()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(
        docs.into_iter()
            .map(|d| DocumentSummary {
                id: d.id,
                title: d.title,
                tags: d.tags,
                last_reviewed: d.last_reviewed.map(|dt| dt.to_string()),
            })
            .collect(),
    ))
}

// ── POST /documents ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateDocumentBody {
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentResponse {
    id: Uuid,
    title: String,
    body: String,
    tags: Vec<String>,
    last_reviewed: Option<String>,
}

pub async fn create_document(
    Extension(storage): Extension<Arc<dyn StorageAdapter>>,
    Extension(llm): Extension<Option<Arc<ModelRouter>>>,
    Json(body): Json<CreateDocumentBody>,
) -> Result<(StatusCode, Json<DocumentResponse>), ApiError> {
    let doc = kaya_core::storage::Document {
        id: Uuid::new_v4(),
        title: body.title,
        body: body.content,
        tags: body.tags,
        owner: None,
        last_reviewed: None,
        related_docs: vec![],
        path: None,
    };

    storage
        .save_document(&doc)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let response = DocumentResponse {
        id: doc.id,
        title: doc.title.clone(),
        body: doc.body.clone(),
        tags: doc.tags.clone(),
        last_reviewed: None,
    };

    if let Some(router) = llm {
        let storage = storage.clone();
        let id = doc.id;
        tokio::spawn(async move {
            if let Err(e) = index_document_chunks(&doc, &storage, &router).await {
                tracing::error!(document_id = %id, error = %e, "reindex failed after create");
            }
        });
    }

    Ok((StatusCode::CREATED, Json(response)))
}

// ── GET /documents/:id ────────────────────────────────────────────────────────

pub async fn get_document(
    Extension(storage): Extension<Arc<dyn StorageAdapter>>,
    Path(id): Path<Uuid>,
) -> Result<Json<DocumentResponse>, ApiError> {
    let doc = storage
        .get_document(id)
        .await
        .map_err(|_| ApiError::not_found(format!("document {id}")))?;

    Ok(Json(DocumentResponse {
        id: doc.id,
        title: doc.title,
        body: doc.body,
        tags: doc.tags,
        last_reviewed: doc.last_reviewed.map(|dt| dt.to_string()),
    }))
}

// ── PUT /documents/:id ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct UpdateDocumentBody {
    pub title: Option<String>,
    pub body: Option<String>,
    pub tags: Option<Vec<String>>,
}

pub async fn update_document(
    Extension(storage): Extension<Arc<dyn StorageAdapter>>,
    Extension(llm): Extension<Option<Arc<ModelRouter>>>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateDocumentBody>,
) -> Result<Json<DocumentResponse>, ApiError> {
    let mut doc = storage
        .get_document(id)
        .await
        .map_err(|_| ApiError::not_found(format!("document {id}")))?;

    if let Some(title) = body.title {
        doc.title = title;
    }
    if let Some(new_body) = body.body {
        doc.body = new_body;
    }
    if let Some(tags) = body.tags {
        doc.tags = tags;
    }

    storage
        .save_document(&doc)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let response = Json(DocumentResponse {
        id: doc.id,
        title: doc.title.clone(),
        body: doc.body.clone(),
        tags: doc.tags.clone(),
        last_reviewed: doc.last_reviewed.map(|dt| dt.to_string()),
    });

    if let Some(router) = llm {
        let storage = storage.clone();
        tokio::spawn(async move {
            if let Err(e) = index_document_chunks(&doc, &storage, &router).await {
                tracing::error!(document_id = %id, error = %e, "reindex failed after update");
            }
        });
    }

    Ok(response)
}

// ── DELETE /documents/:id ─────────────────────────────────────────────────────

pub async fn delete_document(
    Extension(storage): Extension<Arc<dyn StorageAdapter>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    storage
        .get_document(id)
        .await
        .map_err(|_| ApiError::not_found(format!("document {id}")))?;
    storage
        .delete_document(id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

// ── GET /documents/:id/export.pdf ─────────────────────────────────────────────

pub async fn export_document_pdf(
    Extension(storage): Extension<Arc<dyn StorageAdapter>>,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    let doc = storage
        .get_document(id)
        .await
        .map_err(|_| ApiError::not_found(format!("document {id}")))?;

    let pdf = minimal_pdf(&doc.title, &doc.body);
    let filename = sanitize_filename(&doc.title) + ".pdf";

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/pdf")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        )
        .body(Body::from(pdf))
        .unwrap())
}

// ── PDF helpers ───────────────────────────────────────────────────────────────

fn minimal_pdf(title: &str, body: &str) -> Vec<u8> {
    let safe_title = title.replace(['(', ')', '\\', '\n', '\r'], " ");
    let safe_body: String = body
        .chars()
        .take(300)
        .map(|c| if c == '(' || c == ')' || c == '\\' { ' ' } else { c })
        .collect();

    let stream_text = format!(
        "BT /F1 14 Tf 50 750 Td ({safe_title}) Tj 0 -20 Td /F1 10 Tf ({safe_body}) Tj ET"
    );
    let header = format!(
        "%PDF-1.4\n\
         1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n\
         2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n\
         3 0 obj<</Type/Page/MediaBox[0 0 612 792]/Parent 2 0 R\
                   /Resources<</Font<</F1 4 0 R>>>>/Contents 5 0 R>>endobj\n\
         4 0 obj<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>endobj\n"
    );
    let content = format!(
        "5 0 obj\n<</Length {}>>\nstream\n{stream_text}\nendstream\nendobj\n",
        stream_text.len()
    );
    let body_str = header + &content;
    let xref_offset = body_str.len();
    let trailer = format!(
        "\nxref\n0 6\n0000000000 65535 f \n\
         trailer\n<</Size 6/Root 1 0 R>>\nstartxref\n{xref_offset}\n%%EOF"
    );
    (body_str + &trailer).into_bytes()
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect::<String>()
        .to_lowercase()
}
