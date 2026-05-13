"use client";

import Link from "next/link";

type DocumentSummary = {
  id: string;
  title: string;
  tags: string[];
  lastReviewed?: string;
};

type Props = {
  documents: DocumentSummary[];
  loading: boolean;
};

export function DocumentList({ documents, loading }: Props) {
  if (loading) {
    return (
      <div className="p-6 space-y-3">
        {[...Array(4)].map((_, i) => (
          <div key={i} className="h-14 rounded-lg bg-stone-100 animate-pulse" />
        ))}
      </div>
    );
  }

  if (documents.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-64 text-stone-400 text-sm gap-2">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" />
          <polyline points="14 2 14 8 20 8" />
        </svg>
        <p>No documents yet. Ask Kaya to create one.</p>
      </div>
    );
  }

  return (
    <div className="divide-y divide-stone-100">
      {documents.map((doc) => (
        <Link
          key={doc.id}
          href={`/documents/${doc.id}`}
          className="flex items-start gap-4 px-6 py-4 hover:bg-stone-50 transition-colors group"
        >
          <div className="shrink-0 mt-0.5 text-stone-300 group-hover:text-[#1A73E8] transition-colors">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" />
              <polyline points="14 2 14 8 20 8" />
            </svg>
          </div>
          <div className="flex-1 min-w-0">
            <p className="text-sm font-medium text-stone-800 group-hover:text-[#1A73E8] truncate transition-colors">
              {doc.title}
            </p>
            <div className="flex items-center gap-2 mt-1 flex-wrap">
              {doc.tags.map((tag) => (
                <span
                  key={tag}
                  className="px-2 py-0.5 rounded-full bg-stone-100 text-stone-500 text-xs"
                >
                  {tag}
                </span>
              ))}
              {doc.lastReviewed && (
                <span className="text-xs text-stone-400">
                  Reviewed {doc.lastReviewed}
                </span>
              )}
            </div>
          </div>
          <div className="shrink-0 text-stone-300 group-hover:text-stone-400 mt-0.5 transition-colors">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <polyline points="9 18 15 12 9 6" />
            </svg>
          </div>
        </Link>
      ))}
    </div>
  );
}
