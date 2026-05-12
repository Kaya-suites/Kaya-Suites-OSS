"use client";

import { useState } from "react";
import type { ChatSession } from "@/types/chat";

type Props = {
  sessions: ChatSession[];
  currentSessionId: string | null;
  onSelect: (id: string) => void;
  onNew: () => void;
};

export function SessionRail({ sessions, currentSessionId, onSelect, onNew }: Props) {
  const [collapsed, setCollapsed] = useState(false);

  function formatDate(ts: number): string {
    const d = new Date(ts);
    const now = new Date();
    const diffDays = Math.floor((now.getTime() - d.getTime()) / 86400000);
    if (diffDays === 0) return "Today";
    if (diffDays === 1) return "Yesterday";
    return d.toLocaleDateString("en-US", { month: "short", day: "numeric" });
  }

  if (collapsed) {
    return (
      <aside className="flex flex-col items-center w-10 min-h-0 border-r border-stone-200 bg-stone-50 py-3 gap-3 shrink-0">
        <button
          onClick={() => setCollapsed(false)}
          className="p-1.5 rounded hover:bg-stone-200 text-stone-500 hover:text-stone-700 transition-colors"
          title="Expand sessions"
        >
          <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
            <path d="M6 3l5 5-5 5" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </button>
        <button
          onClick={onNew}
          className="p-1.5 rounded hover:bg-stone-200 text-stone-500 hover:text-stone-700 transition-colors"
          title="New conversation"
        >
          <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
            <path d="M8 3v10M3 8h10" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" />
          </svg>
        </button>
      </aside>
    );
  }

  return (
    <aside className="flex flex-col w-56 min-h-0 border-r border-stone-200 bg-stone-50 shrink-0">
      <div className="flex items-center justify-between px-3 py-3 border-b border-stone-200">
        <span className="text-xs font-semibold text-stone-500 uppercase tracking-wider">Sessions</span>
        <div className="flex items-center gap-1">
          <button
            onClick={onNew}
            className="p-1 rounded hover:bg-stone-200 text-stone-500 hover:text-stone-700 transition-colors"
            title="New conversation"
          >
            <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
              <path d="M8 3v10M3 8h10" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" />
            </svg>
          </button>
          <button
            onClick={() => setCollapsed(true)}
            className="p-1 rounded hover:bg-stone-200 text-stone-500 hover:text-stone-700 transition-colors"
            title="Collapse"
          >
            <svg width="14" height="14" viewBox="0 0 16 16">
              <path d="M10 3l-5 5 5 5" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </button>
        </div>
      </div>
      <nav className="flex-1 overflow-y-auto py-2">
        {sessions.length === 0 && (
          <p className="px-3 py-2 text-xs text-stone-400 italic">No past sessions</p>
        )}
        {sessions.map((s) => (
          <button
            key={s.id}
            onClick={() => onSelect(s.id)}
            className={`w-full text-left px-3 py-2 rounded mx-1 text-sm transition-colors ${
              s.id === currentSessionId
                ? "bg-blue-50 text-[#1A73E8] font-medium"
                : "text-stone-700 hover:bg-stone-100"
            }`}
            style={{ width: "calc(100% - 8px)" }}
          >
            <div className="truncate leading-5">{s.title}</div>
            <div className="text-xs text-stone-400 mt-0.5">{formatDate(s.updatedAt)}</div>
          </button>
        ))}
      </nav>
    </aside>
  );
}
