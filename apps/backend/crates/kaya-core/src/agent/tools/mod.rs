//! Concrete tool implementations (FR-13).

mod create_document;
mod delete_document;
mod find_stale_references;
mod list_documents;
mod propose_edit;
mod read_document;
mod search_documents;

pub use create_document::CreateDocument;
pub use delete_document::DeleteDocument;
pub use find_stale_references::FindStaleReferences;
pub use list_documents::ListDocuments;
pub use propose_edit::ProposeEdit;
pub use read_document::ReadDocument;
pub use search_documents::SearchDocuments;

use std::sync::Arc;
use super::tool::Tool;

/// Build the default FR-13 tool set.
pub fn default_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(SearchDocuments),
        Arc::new(ReadDocument),
        Arc::new(ListDocuments),
        Arc::new(CreateDocument),
        Arc::new(DeleteDocument),
        Arc::new(ProposeEdit),
        Arc::new(FindStaleReferences),
    ]
}
