"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
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

  useEffect(() => {
    fetch("/api/documents")
      .then((res) => (res.ok ? res.json() : []))
      .then((data: DocumentSummary[]) => setDocuments(data))
      .catch(() => setDocuments([]))
      .finally(() => setLoading(false));
  }, []);

  return (
    <div className="min-h-screen bg-stone-50">
      {/* Header */}
      <div className="bg-white border-b border-stone-200">
        <div className="max-w-3xl mx-auto px-6 py-4 flex items-center justify-between">
          <div className="flex items-center gap-4">
            <Link
              href="/chat"
              className="flex items-center gap-1.5 text-xs text-stone-400 hover:text-stone-700 transition-colors"
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z" />
              </svg>
              Chat
            </Link>
            <span className="text-stone-200">|</span>
            <h1 className="text-sm font-semibold text-stone-900">Documents</h1>
          </div>
          <span className="text-xs text-stone-400">
            {!loading && `${documents.length} document${documents.length !== 1 ? "s" : ""}`}
          </span>
        </div>
      </div>

      {/* List */}
      <div className="max-w-3xl mx-auto py-4 bg-white rounded-lg mt-4 mx-4 shadow-sm border border-stone-200">
        <DocumentList documents={documents} loading={loading} />
      </div>
    </div>
  );
}
