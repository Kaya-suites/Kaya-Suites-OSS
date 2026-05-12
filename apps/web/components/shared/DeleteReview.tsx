"use client";

import { useState } from "react";
import type { ProposedDelete } from "@/types/chat";

type Props = {
  deletion: ProposedDelete;
  onApprove: (editId: string) => Promise<void>;
  onReject: (editId: string) => void;
};

export function DeleteReview({ deletion, onApprove, onReject }: Props) {
  const [loading, setLoading] = useState(false);

  if (deletion.status === "approved") {
    return (
      <div className="mt-3 rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700 flex items-center gap-2">
        <svg width="14" height="14" viewBox="0 0 16 16" fill="none" className="shrink-0">
          <path d="M3 8l4 4 6-7" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
        Document deleted.
      </div>
    );
  }

  if (deletion.status === "rejected") {
    return (
      <div className="mt-3 rounded-lg border border-stone-200 bg-stone-50 px-4 py-3 text-sm text-stone-400 line-through">
        Deletion rejected.
      </div>
    );
  }

  async function handleApprove() {
    setLoading(true);
    try {
      await onApprove(deletion.id);
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="mt-3 rounded-lg border border-red-200 bg-white overflow-hidden shadow-sm">
      <div className="px-4 py-2.5 border-b border-red-100 bg-red-50 flex items-center gap-2">
        <svg width="14" height="14" viewBox="0 0 16 16" fill="none" className="text-red-500 shrink-0">
          <path d="M2 4h12M6 4V2h4v2M5 4v9a1 1 0 001 1h4a1 1 0 001-1V4" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
        <span className="text-xs font-semibold text-red-600 uppercase tracking-wide">Proposed deletion</span>
      </div>

      <div className="px-4 py-3 border-b border-stone-100">
        <p className="text-sm text-stone-700">
          Delete{" "}
          <span className="font-semibold text-stone-900">{deletion.docTitle}</span>?
          This action cannot be undone.
        </p>
      </div>

      <div className="px-4 py-2.5 flex items-center gap-2 justify-end">
        <button
          onClick={() => onReject(deletion.id)}
          className="px-3 py-1.5 text-sm rounded text-stone-500 hover:text-stone-700 hover:bg-stone-100 transition-colors"
        >
          Reject
        </button>
        <button
          onClick={handleApprove}
          disabled={loading}
          className="px-4 py-1.5 text-sm rounded bg-red-500 text-white font-medium hover:bg-red-600 disabled:opacity-60 transition-colors"
        >
          {loading ? "Deleting…" : "Delete"}
        </button>
      </div>
    </div>
  );
}
