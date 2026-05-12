//! `search_documents` — hybrid semantic + keyword retrieval.

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::agent::{AgentContext, tool::{Tool, ToolOutput}};
use crate::error::KayaError;

pub struct SearchDocuments;

#[async_trait]
impl Tool for SearchDocuments {
    fn name(&self) -> &'static str {
        "search_documents"
    }

    fn description(&self) -> &'static str {
        "Search the knowledge base using semantic similarity. \
         Returns the most relevant document excerpts with their IDs and titles."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["query"],
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query text."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results to return (default 5).",
                    "default": 5
                }
            }
        })
    }

    async fn invoke(&self, input: Value, ctx: &AgentContext) -> Result<ToolOutput, KayaError> {
        let query = input["query"]
            .as_str()
            .ok_or_else(|| KayaError::Internal("search_documents: missing 'query'".into()))?
            .to_owned();
        let limit = input["limit"].as_u64().unwrap_or(5) as usize;

        // Embed the query and run vector search.
        let emb = ctx.router.embed(query.clone()).await?;
        let hits = ctx.storage.search_embeddings(&emb.embedding, limit).await?;

        // Fetch full documents for each hit, deduplicate by document ID.
        let mut seen = std::collections::HashSet::new();
        let mut results = Vec::new();
        for hit in &hits {
            if !seen.insert(hit.document_id) {
                continue;
            }
            if let Ok(doc) = ctx.storage.get_document(hit.document_id).await {
                let excerpt_end = doc.body.len().min(300);
                results.push(json!({
                    "id": doc.id,
                    "title": doc.title,
                    "chunk_index": hit.chunk_index,
                    "excerpt": &doc.body[..excerpt_end],
                }));
            }
        }

        // Keyword fallback when the embedding index is empty.
        if results.is_empty() {
            let query_lower = query.to_lowercase();
            let all_docs = ctx.storage.list_documents().await?;
            for doc in all_docs.into_iter().take(limit) {
                if doc.title.to_lowercase().contains(&query_lower)
                    || doc.body.to_lowercase().contains(&query_lower)
                {
                    let excerpt_end = doc.body.len().min(300);
                    results.push(json!({
                        "id": doc.id,
                        "title": doc.title,
                        "chunk_index": 0,
                        "excerpt": &doc.body[..excerpt_end],
                    }));
                }
            }
        }

        Ok(ToolOutput::value(json!({ "documents": results })))
    }
}
