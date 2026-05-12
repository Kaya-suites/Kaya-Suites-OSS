//! Paragraph-level diff between two Markdown documents.
//!
//! Documents are split on blank lines (`\n\n`). Each paragraph gets a stable
//! ID (`"p0"`, `"p1"`, …) keyed to its position in the *old* document.
//! New paragraphs inserted between existing ones get synthetic IDs (`"n0"`,
//! `"n1"`, …).
//!
//! The same representation is consumed by the UI diff renderer in Prompt 5.

use serde::{Deserialize, Serialize};

// ── Public types ─────────────────────────────────────────────────────────────

/// A paragraph-level diff between two document bodies.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ParagraphDiff {
    pub changes: Vec<ParagraphChange>,
}

impl ParagraphDiff {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

/// A single change in a paragraph-level diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ParagraphChange {
    /// A paragraph was added to the new document.
    Add {
        /// Synthetic ID for this new paragraph.
        paragraph_id: String,
        text: String,
        /// ID of the paragraph this was inserted *after*, or `None` if it
        /// was prepended before all existing paragraphs.
        after_id: Option<String>,
    },
    /// A paragraph was removed from the old document.
    Remove {
        paragraph_id: String,
        text: String,
    },
    /// A paragraph was present in both documents but its text changed.
    Modify {
        paragraph_id: String,
        old_text: String,
        new_text: String,
    },
}

// ── Algorithm ────────────────────────────────────────────────────────────────

/// Compute a paragraph-level diff between `old_body` and `new_body`.
pub fn compute_paragraph_diff(old_body: &str, new_body: &str) -> ParagraphDiff {
    let old_paras: Vec<&str> = split_paragraphs(old_body);
    let new_paras: Vec<&str> = split_paragraphs(new_body);

    // LCS of (old_index, new_index) pairs where text is identical.
    let lcs = lcs_indices(&old_paras, &new_paras);

    let mut changes = Vec::new();
    let mut new_id_counter = 0usize;

    let mut old_i = 0usize;
    let mut new_j = 0usize;
    let mut last_old_id: Option<String> = None;

    for (lcs_old, lcs_new) in &lcs {
        // Emit changes for old[old_i .. lcs_old] and new[new_j .. lcs_new].
        let old_gap = &old_paras[old_i..*lcs_old];
        let new_gap = &new_paras[new_j..*lcs_new];

        emit_gap(
            old_gap,
            new_gap,
            old_i,
            &last_old_id,
            &mut new_id_counter,
            &mut changes,
        );

        old_i = *lcs_old;
        new_j = *lcs_new;
        // The matched paragraph is unchanged — advance both.
        last_old_id = Some(format!("p{old_i}"));
        old_i += 1;
        new_j += 1;
    }

    // Tail: old[old_i..] and new[new_j..]
    emit_gap(
        &old_paras[old_i..],
        &new_paras[new_j..],
        old_i,
        &last_old_id,
        &mut new_id_counter,
        &mut changes,
    );

    ParagraphDiff { changes }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn split_paragraphs(body: &str) -> Vec<&str> {
    body.split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect()
}

/// Emit [`ParagraphChange`]s for a gap between two LCS anchors.
///
/// - 1 old + 1 new → `Modify`
/// - Otherwise → `Remove` each old, then `Add` each new
fn emit_gap(
    old_gap: &[&str],
    new_gap: &[&str],
    old_base: usize,
    last_old_id: &Option<String>,
    new_id_counter: &mut usize,
    changes: &mut Vec<ParagraphChange>,
) {
    if old_gap.len() == 1 && new_gap.len() == 1 {
        // Treat as an in-place modification.
        changes.push(ParagraphChange::Modify {
            paragraph_id: format!("p{old_base}"),
            old_text: old_gap[0].to_owned(),
            new_text: new_gap[0].to_owned(),
        });
    } else {
        // Remove each old paragraph.
        for (k, text) in old_gap.iter().enumerate() {
            changes.push(ParagraphChange::Remove {
                paragraph_id: format!("p{}", old_base + k),
                text: (*text).to_owned(),
            });
        }
        // Add each new paragraph.
        let mut prev = last_old_id.clone();
        // If we removed some paragraphs, the last "anchor" is the final old removal.
        // For the add position hint we stay at the last anchor before this gap.
        for text in new_gap {
            let nid = format!("n{new_id_counter}");
            *new_id_counter += 1;
            changes.push(ParagraphChange::Add {
                paragraph_id: nid.clone(),
                text: (*text).to_owned(),
                after_id: prev.clone(),
            });
            prev = Some(nid);
        }
    }
}

/// Longest Common Subsequence on slices of `&str`, returning matched index
/// pairs `(old_i, new_j)`.
fn lcs_indices<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<(usize, usize)> {
    let n = old.len();
    let m = new.len();
    // dp[i][j] = LCS length for old[..i] vs new[..j]
    let mut dp = vec![vec![0u16; m + 1]; n + 1];
    for i in 1..=n {
        for j in 1..=m {
            dp[i][j] = if old[i - 1] == new[j - 1] {
                dp[i - 1][j - 1] + 1
            } else {
                dp[i - 1][j].max(dp[i][j - 1])
            };
        }
    }
    // Backtrack
    let mut pairs = Vec::new();
    let (mut i, mut j) = (n, m);
    while i > 0 && j > 0 {
        if old[i - 1] == new[j - 1] {
            pairs.push((i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] >= dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    pairs.reverse();
    pairs
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_documents_produce_no_changes() {
        let body = "Para one.\n\nPara two.\n\nPara three.";
        let diff = compute_paragraph_diff(body, body);
        assert!(diff.is_empty(), "{diff:?}");
    }

    #[test]
    fn added_paragraph_detected() {
        let old = "Para one.\n\nPara three.";
        let new = "Para one.\n\nPara two.\n\nPara three.";
        let diff = compute_paragraph_diff(old, new);
        assert_eq!(diff.changes.len(), 1);
        assert!(matches!(&diff.changes[0], ParagraphChange::Add { text, .. } if text == "Para two."));
    }

    #[test]
    fn removed_paragraph_detected() {
        let old = "Para one.\n\nPara two.\n\nPara three.";
        let new = "Para one.\n\nPara three.";
        let diff = compute_paragraph_diff(old, new);
        assert_eq!(diff.changes.len(), 1);
        assert!(matches!(&diff.changes[0], ParagraphChange::Remove { text, .. } if text == "Para two."));
    }

    #[test]
    fn modified_paragraph_detected() {
        let old = "Para one.\n\nOld text.\n\nPara three.";
        let new = "Para one.\n\nNew text.\n\nPara three.";
        let diff = compute_paragraph_diff(old, new);
        assert_eq!(diff.changes.len(), 1);
        assert!(matches!(
            &diff.changes[0],
            ParagraphChange::Modify { old_text, new_text, .. }
                if old_text == "Old text." && new_text == "New text."
        ));
    }
}
