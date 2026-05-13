"use client";

import { useEffect, useState } from "react";
import { use } from "react";
import Link from "next/link";
import { DocumentEditor } from "@/components/shared/DocumentEditor";
import type { KayaDocument } from "@/types/chat";

export default function DocumentEditorPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = use(params);
  const [doc, setDoc] = useState<KayaDocument | null>(null);
  const [error, setError] = useState(false);

  useEffect(() => {
    fetch(`/api/documents/${id}`)
      .then((res) => (res.ok ? res.json() : Promise.reject(res.status)))
      .then((data: KayaDocument) => setDoc(data))
      .catch(() => setError(true));
  }, [id]);

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center h-screen bg-stone-50 gap-3 text-stone-500">
        <p className="text-sm">Document not found.</p>
        <Link href="/documents" className="text-xs text-[#1A73E8] hover:underline">
          ← Back to Documents
        </Link>
      </div>
    );
  }

  if (!doc) {
    return (
      <div className="flex flex-col h-screen bg-white">
        <div className="flex items-center gap-3 px-6 py-3 border-b border-stone-200">
          <div className="h-4 w-24 bg-stone-100 rounded animate-pulse" />
          <div className="flex-1 h-5 bg-stone-100 rounded animate-pulse" />
          <div className="h-8 w-16 bg-stone-100 rounded animate-pulse" />
        </div>
        <div className="flex-1 px-6 py-4 space-y-3">
          {[...Array(6)].map((_, i) => (
            <div key={i} className="h-4 bg-stone-100 rounded animate-pulse" style={{ width: `${70 + (i % 3) * 10}%` }} />
          ))}
        </div>
      </div>
    );
  }

  return <DocumentEditor doc={doc} />;
}
