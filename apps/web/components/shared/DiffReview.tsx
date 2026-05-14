"use client";

import { useState } from "react";
import { wordDiff } from "@/lib/diff";
import type { ProposedEdit } from "@/types/chat";

type Props = {
  edit: ProposedEdit;
  editedText: string;
  onTextChange: (editId: string, text: string) => void;
  onApprove: (editId: string, finalText: string) => Promise<void>;
  onReject: (editId: string) => void;
};

export function DiffReview({ edit, editedText, onTextChange, onApprove, onReject }: Props) {
  const [loading, setLoading] = useState(false);

  if (edit.status === "approved") {
    return (
      <div className="mt-3 rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-700 flex items-center gap-2">
        <svg width="14" height="14" viewBox="0 0 16 16" fill="none" className="shrink-0">
          <path d="M3 8l4 4 6-7" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
        Edit approved and committed to the document.
      </div>
    );
  }

  if (edit.status === "rejected") {
    return (
      <div className="mt-3 rounded-lg border border-stone-200 bg-stone-50 px-4 py-3 text-sm text-stone-400 line-through">
        Edit rejected.
      </div>
    );
  }

  const diff = wordDiff(edit.original, editedText);

  async function handleApprove() {
    setLoading(true);
    try {
      await onApprove(edit.id, editedText);
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="mt-3 rounded-lg border border-stone-200 bg-white overflow-hidden shadow-sm">
      <div className="px-4 py-2.5 border-b border-stone-100 bg-stone-50 flex items-center gap-2">
        <svg width="14" height="14" viewBox="0 0 16 16" fill="none" className="text-[#1A73E8] shrink-0">
          <rect x="1" y="3" width="14" height="10" rx="2" stroke="currentColor" strokeWidth="1.4" />
          <path d="M5 7h6M5 9.5h4" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
        </svg>
        <span className="text-xs font-semibold text-stone-600 uppercase tracking-wide">Proposed edit</span>
      </div>

      {/* Diff view */}
      <div className="px-4 py-3 font-mono text-sm leading-relaxed border-b border-stone-100">
        <div className="mb-1.5 text-xs text-stone-400 uppercase tracking-wide font-sans">Changes</div>
        <p className="whitespace-pre-wrap">
          {diff.map((op, i) => {
            if (op.type === "equal") {
              return <span key={i}>{op.text}</span>;
            }
            if (op.type === "delete") {
              return (
                <span key={i} className="bg-red-100 text-red-700 line-through rounded px-0.5">
                  {op.text}
                </span>
              );
            }
            return (
              <span key={i} className="bg-emerald-100 text-emerald-700 rounded px-0.5">
                {op.text}
              </span>
            );
          })}
        </p>
      </div>

      {/* Editable proposed text (FR-16) */}
      <div className="px-4 py-3 border-b border-stone-100">
        <div className="mb-1.5 text-xs text-stone-400 uppercase tracking-wide font-sans">Edit before approving</div>
        <textarea
          value={editedText}
          onChange={(e) => onTextChange(edit.id, e.target.value)}
          rows={3}
          className="w-full text-sm font-mono border border-stone-200 rounded px-3 py-2 focus:outline-none focus:ring-2 focus:ring-[#1A73E8] focus:border-transparent resize-y bg-white text-stone-800 leading-relaxed"
        />
      </div>

      {/* Actions */}
      <div className="px-4 py-2.5 flex items-center gap-2 justify-end">
        <button
          onClick={() => onReject(edit.id)}
          className="px-3 py-1.5 text-sm rounded text-stone-500 hover:text-stone-700 hover:bg-stone-100 transition-colors"
        >
          Reject
        </button>
        <button
          onClick={handleApprove}
          disabled={loading}
          className="px-4 py-1.5 text-sm rounded bg-[#1A73E8] text-white font-medium hover:bg-[#1557B0] disabled:opacity-60 transition-colors"
        >
          {loading ? "Approving…" : "Approve"}
        </button>
      </div>
    </div>
  );
}
