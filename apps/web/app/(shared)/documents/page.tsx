"use client";

import { useEffect, useRef, useState } from "react";
import { DocumentList } from "@/components/shared/DocumentList";

type DocumentSummary = {
  id: string;
  title: string;
  tags: string[];
  lastReviewed?: string;
};

export default function DocumentsPage() {
  const [documents, setDocuments] = useState<DocumentSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [showForm, setShowForm] = useState(false);
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const titleRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    fetch("/api/documents")
      .then((res) => (res.ok ? res.json() : []))
      .then((data: DocumentSummary[]) => setDocuments(data))
      .catch(() => setDocuments([]))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    if (showForm) titleRef.current?.focus();
  }, [showForm]);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!title.trim() || !content.trim()) return;
    setSubmitting(true);
    setError(null);
    try {
      const res = await fetch("/api/documents", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ title: title.trim(), content: content.trim() }),
      });
      if (!res.ok) {
        const data = await res.json().catch(() => null) as { error?: string } | null;
        setError(data?.error ?? `Error ${res.status}`);
        return;
      }
      const created = await res.json() as DocumentSummary;
      setDocuments((prev) => [created, ...prev]);
      setTitle("");
      setContent("");
      setShowForm(false);
    } catch {
      setError("Could not reach the backend.");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="h-full overflow-y-auto bg-stone-50">
      {/* Header */}
      <div className="bg-white border-b border-stone-200">
        <div className="max-w-3xl mx-auto px-6 py-4 flex items-center justify-between">
          <h1 className="text-sm font-semibold text-stone-900">Documents</h1>
          <div className="flex items-center gap-3">
            <span className="text-xs text-stone-400">
              {!loading && `${documents.length} document${documents.length !== 1 ? "s" : ""}`}
            </span>
            <button
              onClick={() => { setShowForm((v) => !v); setError(null); }}
              className="text-xs px-3 py-1.5 bg-stone-800 text-white rounded-md hover:bg-stone-700 transition-colors"
            >
              {showForm ? "Cancel" : "Import"}
            </button>
          </div>
        </div>
      </div>

      {/* Import form */}
      {showForm && (
        <div className="max-w-3xl mx-auto mt-4 px-4">
          <form
            onSubmit={handleSubmit}
            className="bg-white rounded-lg border border-stone-200 shadow-sm p-5 space-y-3"
          >
            <input
              ref={titleRef}
              type="text"
              placeholder="Document title"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              className="w-full text-sm border border-stone-200 rounded-md px-3 py-2 focus:outline-none focus:ring-1 focus:ring-stone-400 placeholder:text-stone-400"
              required
            />
            <textarea
              placeholder="Paste Markdown content here…"
              value={content}
              onChange={(e) => setContent(e.target.value)}
              rows={10}
              className="w-full text-sm font-mono border border-stone-200 rounded-md px-3 py-2 focus:outline-none focus:ring-1 focus:ring-stone-400 placeholder:text-stone-400 resize-y"
              required
            />
            {error && <p className="text-xs text-red-500">{error}</p>}
            <div className="flex justify-end">
              <button
                type="submit"
                disabled={submitting || !title.trim() || !content.trim()}
                className="text-xs px-4 py-2 bg-stone-800 text-white rounded-md hover:bg-stone-700 disabled:opacity-50 transition-colors"
              >
                {submitting ? "Saving…" : "Save document"}
              </button>
            </div>
          </form>
        </div>
      )}

      {/* List */}
      <div className="max-w-3xl mx-auto py-4 bg-white rounded-lg mt-4 mx-4 shadow-sm border border-stone-200">
        <DocumentList documents={documents} loading={loading} />
      </div>
    </div>
  );
}
