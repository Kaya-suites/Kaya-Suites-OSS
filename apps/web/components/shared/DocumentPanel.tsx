"use client";

import { useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { KayaDocument } from "@/types/chat";

type Props = {
  docId: string | null;
  scrollToParagraphId?: string | null;
  refreshKey?: number;
  onClose: () => void;
};

export function DocumentPanel({ docId, scrollToParagraphId, refreshKey, onClose }: Props) {
  const [doc, setDoc] = useState<KayaDocument | null>(null);
  const [loading, setLoading] = useState(false);
  const [downloading, setDownloading] = useState(false);
  const contentRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!docId) {
      setDoc(null);
      return;
    }
    setLoading(true);
    fetch(`/api/documents/${docId}`)
      .then((r) => r.json())
      .then((data: KayaDocument) => setDoc(data))
      .catch(() => setDoc(null))
      .finally(() => setLoading(false));
  }, [docId, refreshKey]);

  // Scroll to cited paragraph
  useEffect(() => {
    if (!scrollToParagraphId || !contentRef.current) return;
    // Paragraph IDs are positional (p-1, p-2…); map to nth <p> element
    const match = scrollToParagraphId.match(/^p-(\d+)$/);
    if (!match) return;
    const idx = parseInt(match[1], 10) - 1;
    const paragraphs = contentRef.current.querySelectorAll("p, h2, h3");
    const target = paragraphs[idx] as HTMLElement | undefined;
    if (target) {
      target.scrollIntoView({ behavior: "smooth", block: "start" });
      target.classList.add("ring-2", "ring-[#1A73E8]", "ring-offset-2", "rounded");
      setTimeout(() => target.classList.remove("ring-2", "ring-[#1A73E8]", "ring-offset-2", "rounded"), 2000);
    }
  }, [scrollToParagraphId, doc]);

  async function handleExport() {
    if (!docId || !doc) return;
    setDownloading(true);
    try {
      const res = await fetch(`/api/documents/${docId}/export`);
      if (!res.ok) throw new Error("export failed");
      const blob = await res.blob();
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${doc.title.replace(/[^a-z0-9]/gi, "_").toLowerCase()}.pdf`;
      document.body.appendChild(a);
      a.click();
      a.remove();
      URL.revokeObjectURL(url);
    } catch {
      // silent fail for demo
    } finally {
      setDownloading(false);
    }
  }

  if (!docId) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-stone-400 gap-3 p-8">
        <svg width="40" height="40" viewBox="0 0 40 40" fill="none" className="opacity-40">
          <rect x="8" y="5" width="24" height="30" rx="3" stroke="currentColor" strokeWidth="1.5" />
          <path d="M14 14h12M14 19h12M14 24h8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        </svg>
        <p className="text-sm text-center leading-relaxed">
          Documents appear here when the agent cites one,<br />or when you click a citation chip.
        </p>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full min-h-0">
      {/* Toolbar */}
      <div className="flex items-center justify-between px-5 py-3 border-b border-stone-200 bg-white shrink-0">
        <h2 className="text-sm font-semibold text-stone-800 truncate pr-4">
          {loading ? "Loading…" : (doc?.title ?? "Document")}
        </h2>
        <div className="flex items-center gap-1 shrink-0">
          {doc && (
            <button
              onClick={handleExport}
              disabled={downloading}
              className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-md border border-stone-200 text-stone-600 hover:border-[#1A73E8] hover:text-[#1A73E8] hover:bg-blue-50 transition-colors disabled:opacity-50"
              title="Export as PDF"
            >
              <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
                <path d="M8 2v9M4 8l4 4 4-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
                <path d="M2 13h12" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
              </svg>
              {downloading ? "Exporting…" : "Export PDF"}
            </button>
          )}
          <button
            onClick={onClose}
            className="p-1.5 rounded hover:bg-stone-100 text-stone-400 hover:text-stone-600 transition-colors ml-1"
            title="Close document"
          >
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
              <path d="M3 3l10 10M13 3L3 13" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
            </svg>
          </button>
        </div>
      </div>

      {/* Tags */}
      {doc?.tags && doc.tags.length > 0 && (
        <div className="flex flex-wrap gap-1.5 px-5 pt-3 pb-0 shrink-0">
          {doc.tags.map((tag) => (
            <span
              key={tag}
              className="px-2 py-0.5 rounded-full bg-stone-100 text-stone-500 text-xs"
            >
              {tag}
            </span>
          ))}
        </div>
      )}

      {/* Content */}
      <div ref={contentRef} className="flex-1 overflow-y-auto px-8 py-6 min-h-0">
        {loading && (
          <div className="flex items-center gap-2 text-stone-400 text-sm">
            <div className="w-4 h-4 border-2 border-stone-300 border-t-[#1A73E8] rounded-full animate-spin" />
            Loading document…
          </div>
        )}
        {!loading && doc && (
          <div className="prose prose-stone max-w-none prose-headings:font-semibold prose-headings:text-stone-900 prose-p:text-stone-700 prose-p:leading-relaxed prose-p:mb-4 prose-li:text-stone-700 prose-strong:text-stone-900 prose-code:text-stone-800 prose-code:bg-stone-100 prose-code:rounded prose-code:px-1 prose-code:text-sm prose-pre:bg-stone-100 prose-pre:rounded-lg prose-table:text-sm">
            <ReactMarkdown remarkPlugins={[remarkGfm]}>{doc.body}</ReactMarkdown>
          </div>
        )}
      </div>
    </div>
  );
}
