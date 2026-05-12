//! Frontmatter parsing and Markdown serialisation for knowledge-base documents.
//!
//! File format:
//! ```text
//! ---
//! id: <uuid>
//! title: My Document
//! owner: alice           # optional
//! last_reviewed: 2024-01-15  # optional, ISO date
//! tags: [rust, testing]  # optional
//! related_docs: []       # optional, UUID list
//! ---
//!
//! Raw Markdown body here.
//! ```
//!
//! UUIDs that are absent from the frontmatter (hand-written files) are generated
//! on first parse; the caller is responsible for writing the updated file back so
//! the UUID is persisted.

use chrono::NaiveDate;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use kaya_core::storage::Document;

// ── Error ─────────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("missing or malformed YAML frontmatter")]
    MissingFrontmatter,
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("frontmatter is missing required field `title`")]
    MissingTitle,
}

// ── Private YAML structs ───────────────────────────────────────────────────────

/// Deserialised from YAML. `id` is optional so hand-written files are accepted.
#[derive(Debug, serde::Deserialize)]
struct RawFrontmatter {
    #[serde(default)]
    id: Option<Uuid>,
    title: Option<String>,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    last_reviewed: Option<NaiveDate>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    related_docs: Vec<Uuid>,
}

/// Serialised to YAML when writing a file. `id` is always present.
#[derive(Debug, serde::Serialize)]
struct WriteFrontmatter<'a> {
    id: Uuid,
    title: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_reviewed: Option<NaiveDate>,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    tags: &'a [String],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    related_docs: &'a [Uuid],
}

// ── Public API ─────────────────────────────────────────────────────────────────

/// Parse a raw Markdown+frontmatter string into a [`Document`].
///
/// Returns `(document, id_was_generated)`. When `id_was_generated` is `true`
/// the caller should write the serialised document back to disk so the UUID is
/// preserved for future reads (FR-2 stability).
pub fn parse_document(raw: &str) -> Result<(Document, bool), ParseError> {
    let (yaml, body) = split_frontmatter(raw).ok_or(ParseError::MissingFrontmatter)?;
    let rfm: RawFrontmatter = serde_yaml::from_str(yaml)?;

    let title = rfm.title.ok_or(ParseError::MissingTitle)?;
    let id_was_generated = rfm.id.is_none();
    let id = rfm.id.unwrap_or_else(Uuid::new_v4);

    let doc = Document {
        id,
        title,
        owner: rfm.owner,
        last_reviewed: rfm.last_reviewed,
        tags: rfm.tags,
        related_docs: rfm.related_docs,
        body: body.to_string(),
        path: None,
    };
    Ok((doc, id_was_generated))
}

/// Serialise a [`Document`] to the Markdown+frontmatter format used on disk.
pub fn to_markdown(doc: &Document) -> Result<String, serde_yaml::Error> {
    let fm = WriteFrontmatter {
        id: doc.id,
        title: &doc.title,
        owner: doc.owner.as_deref(),
        last_reviewed: doc.last_reviewed,
        tags: &doc.tags,
        related_docs: &doc.related_docs,
    };
    // serde_yaml::to_string produces plain YAML with a trailing newline.
    let yaml = serde_yaml::to_string(&fm)?;
    Ok(format!("---\n{}---\n\n{}", yaml, doc.body))
}

/// Compute the SHA-256 hex digest of raw bytes.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    format!("{:x}", h.finalize())
}

// ── Frontmatter splitting ──────────────────────────────────────────────────────

/// Split a raw string into `(yaml_block, body)`.
///
/// The file must start with `---\n` (or `---\r\n`). The closing `---` must
/// appear at the start of a line. The body is everything after the closing
/// delimiter, with a single leading newline stripped.
fn split_frontmatter(raw: &str) -> Option<(&str, &str)> {
    // Strip opening ---
    let raw = raw.strip_prefix("---")?;
    let raw = raw.strip_prefix('\n').or_else(|| raw.strip_prefix("\r\n"))?;

    // Find closing --- on its own line
    // We look for "\n---" followed by \n, \r\n, or end-of-string.
    let close = find_close_marker(raw)?;
    let yaml = &raw[..close];
    let after_marker = &raw[close + "\n---".len()..];
    // Strip the newline that follows --- and an optional blank separator line,
    // so `body` does not include the blank line between the closing --- and
    // the document content.  `to_markdown` always re-emits that blank line.
    let after_nl = after_marker
        .strip_prefix('\n')
        .or_else(|| after_marker.strip_prefix("\r\n"))
        .unwrap_or(after_marker);
    let body = after_nl
        .strip_prefix('\n')
        .or_else(|| after_nl.strip_prefix("\r\n"))
        .unwrap_or(after_nl);
    Some((yaml, body))
}

fn find_close_marker(s: &str) -> Option<usize> {
    let mut search = s;
    let mut offset = 0;
    loop {
        let pos = search.find("\n---")?;
        let after = &search[pos + "\n---".len()..];
        if after.is_empty() || after.starts_with('\n') || after.starts_with('\r') {
            return Some(offset + pos);
        }
        // Not a real closing marker (e.g. "---inside-word"), keep searching.
        offset += pos + 1;
        search = &search[pos + 1..];
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
---
id: 550e8400-e29b-41d4-a716-446655440000
title: Test Document
owner: alice
last_reviewed: 2024-01-15
tags:
  - rust
  - testing
related_docs: []
---

# Hello

Body text.
";

    #[test]
    fn round_trip_parse_serialize() {
        let (doc, generated) = parse_document(SAMPLE).unwrap();
        assert!(!generated, "id was present in YAML, should not be generated");
        assert_eq!(doc.title, "Test Document");
        assert_eq!(doc.owner.as_deref(), Some("alice"));
        assert_eq!(
            doc.last_reviewed,
            Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap())
        );
        assert_eq!(doc.tags, ["rust", "testing"]);
        assert!(doc.related_docs.is_empty());
        assert!(doc.body.contains("Body text."));

        let serialised = to_markdown(&doc).unwrap();
        let (doc2, _) = parse_document(&serialised).unwrap();
        assert_eq!(doc.id, doc2.id);
        assert_eq!(doc.title, doc2.title);
        assert_eq!(doc.last_reviewed, doc2.last_reviewed);
        assert_eq!(doc.tags, doc2.tags);
        assert_eq!(doc.body.trim(), doc2.body.trim());
    }

    #[test]
    fn missing_id_generates_one() {
        let raw = "---\ntitle: No ID Here\n---\n\nBody.\n";
        let (doc, generated) = parse_document(raw).unwrap();
        assert!(generated);
        assert_eq!(doc.title, "No ID Here");
    }

    #[test]
    fn body_containing_triple_dash_is_not_split() {
        let raw = "---\nid: 550e8400-e29b-41d4-a716-446655440000\ntitle: T\n---\n\nLine 1\n---\nLine 2\n";
        let (doc, _) = parse_document(raw).unwrap();
        assert!(doc.body.contains("---"), "body should preserve inner --- lines");
    }
}
