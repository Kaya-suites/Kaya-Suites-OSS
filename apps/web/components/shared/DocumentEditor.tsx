"use client";

import { useState, useEffect, useRef } from "react";
import dynamic from "next/dynamic";
import Link from "next/link";
import type { MDXEditorMethods } from "@mdxeditor/editor";
import type { KayaDocument } from "@/types/chat";

const MDXEditorClient = dynamic(
  () => import("./MDXEditorClient").then((m) => m.MDXEditorClient),
  { ssr: false },
);

type SaveStatus = "idle" | "saving" | "saved" | "error";

type Props = {
  doc: KayaDocument;
};

export function DocumentEditor({ doc }: Props) {
  const [title, setTitle] = useState(doc.title);
  const [body, setBody] = useState(doc.body);
  const [tagsInput, setTagsInput] = useState(doc.tags.join(", "));
  const [status, setStatus] = useState<SaveStatus>("idle");
  const editorRef = useRef<MDXEditorMethods>(null);
  const savedTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const isDirty =
    title !== doc.title ||
    body !== doc.body ||
    tagsInput !== doc.tags.join(", ");

  async function handleSave() {
    setStatus("saving");
    const tags = tagsInput
      .split(",")
      .map((t) => t.trim())
      .filter(Boolean);

    try {
      const res = await fetch(`/api/documents/${doc.id}`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ title, body, tags }),
      });
      if (!res.ok) {
        setStatus("error");
      } else {
        setStatus("saved");
        if (savedTimer.current) clearTimeout(savedTimer.current);
        savedTimer.current = setTimeout(() => setStatus("idle"), 2500);
      }
    } catch {
      setStatus("error");
    }
  }

  useEffect(() => {
    return () => {
      if (savedTimer.current) clearTimeout(savedTimer.current);
    };
  }, []);

  return (
    <div className="flex flex-col h-screen bg-white">
      {/* Top bar */}
      <div className="flex items-center gap-3 px-6 py-3 border-b border-stone-200 shrink-0">
        <Link
          href="/documents"
          className="flex items-center gap-1 text-xs text-stone-400 hover:text-stone-700 transition-colors"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="15 18 9 12 15 6" />
          </svg>
          Documents
        </Link>

        <div className="flex-1 min-w-0">
          <input
            type="text"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            className="w-full text-base font-semibold text-stone-900 bg-transparent border-none outline-none placeholder-stone-300 focus:ring-0"
            placeholder="Untitled"
          />
        </div>

        <div className="flex items-center gap-2 shrink-0">
          {status === "saved" && (
            <span className="text-xs text-emerald-600 flex items-center gap-1">
              <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
                <path d="M3 8l4 4 6-7" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
              Saved
            </span>
          )}
          {status === "error" && (
            <span className="text-xs text-red-500">Save failed</span>
          )}
          <button
            onClick={handleSave}
            disabled={!isDirty || status === "saving"}
            className="px-3 py-1.5 text-sm rounded bg-[#1A73E8] text-white font-medium hover:bg-[#1557B0] disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
          >
            {status === "saving" ? "Saving…" : "Save"}
          </button>
        </div>
      </div>

      {/* Tags row */}
      <div className="flex items-center gap-2 px-6 py-2 border-b border-stone-100 shrink-0">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="text-stone-300 shrink-0">
          <path d="M20.59 13.41l-7.17 7.17a2 2 0 01-2.83 0L2 12V2h10l8.59 8.59a2 2 0 010 2.82z" />
          <line x1="7" y1="7" x2="7.01" y2="7" />
        </svg>
        <input
          type="text"
          value={tagsInput}
          onChange={(e) => setTagsInput(e.target.value)}
          placeholder="Add tags, comma-separated…"
          className="flex-1 text-xs text-stone-500 bg-transparent border-none outline-none placeholder-stone-300 focus:ring-0"
        />
      </div>

      {/* MDX Editor */}
      <div className="flex-1 overflow-y-auto [&_.mdxeditor]:h-full [&_.mdxeditor-root-contenteditable]:min-h-full">
        <MDXEditorClient
          markdown={body}
          onChange={setBody}
          editorRef={editorRef}
        />
      </div>
    </div>
  );
}
