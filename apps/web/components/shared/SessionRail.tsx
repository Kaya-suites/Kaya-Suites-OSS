"use client";

import { useEffect, useRef, useState } from "react";
import type { ChatSession } from "@/types/chat";

type ViewMode = "comfortable" | "compact";

type Props = {
  sessions: ChatSession[];
  currentSessionId: string | null;
  onSelect: (id: string) => void;
  onNew: () => void;
  onRename?: (id: string, title: string) => void;
};

function useViewMode(): [ViewMode, (m: ViewMode) => void] {
  const [mode, setMode] = useState<ViewMode>("comfortable");

  useEffect(() => {
    const saved = localStorage.getItem("session-view-mode") as ViewMode | null;
    if (saved === "comfortable" || saved === "compact") setMode(saved);
  }, []);

  function save(m: ViewMode) {
    localStorage.setItem("session-view-mode", m);
    setMode(m);
  }

  return [mode, save];
}

function formatDate(ts: number): string {
  const d = new Date(ts);
  const now = new Date();
  const diffDays = Math.floor((now.getTime() - d.getTime()) / 86400000);
  if (diffDays === 0) return "Today";
  if (diffDays === 1) return "Yesterday";
  return d.toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

export function SessionRail({ sessions, currentSessionId, onSelect, onNew, onRename }: Props) {
  const [collapsed, setCollapsed] = useState(false);
  const [viewMode, setViewMode] = useViewMode();
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editValue, setEditValue] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (editingId && inputRef.current) inputRef.current.focus();
  }, [editingId]);

  function startEdit(s: ChatSession, e: React.MouseEvent) {
    e.stopPropagation();
    setEditingId(s.id);
    setEditValue(s.title);
  }

  function commitEdit(id: string) {
    const trimmed = editValue.trim();
    if (trimmed) onRename?.(id, trimmed);
    setEditingId(null);
  }

  function handleEditKey(e: React.KeyboardEvent, id: string) {
    if (e.key === "Enter") commitEdit(id);
    if (e.key === "Escape") setEditingId(null);
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
          {/* View toggle */}
          <button
            onClick={() => setViewMode(viewMode === "comfortable" ? "compact" : "comfortable")}
            className="p-1 rounded hover:bg-stone-200 text-stone-500 hover:text-stone-700 transition-colors"
            title={viewMode === "comfortable" ? "Switch to compact view" : "Switch to comfortable view"}
          >
            {viewMode === "comfortable" ? (
              <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
                <rect x="2" y="3" width="12" height="2" rx="1" fill="currentColor" />
                <rect x="2" y="7" width="12" height="2" rx="1" fill="currentColor" />
                <rect x="2" y="11" width="12" height="2" rx="1" fill="currentColor" />
              </svg>
            ) : (
              <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
                <rect x="2" y="2" width="12" height="3" rx="1" fill="currentColor" />
                <rect x="2" y="7" width="12" height="3" rx="1" fill="currentColor" />
                <rect x="2" y="12" width="8" height="2" rx="1" fill="currentColor" />
              </svg>
            )}
          </button>
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
        {sessions.map((s, i) => {
          const isActive = s.id === currentSessionId;
          const isEditing = editingId === s.id;

          if (viewMode === "compact") {
            return (
              <div
                key={s.id ?? i}
                className={`group flex items-center mx-1 rounded transition-colors ${
                  isActive ? "bg-blue-50 text-[#1A73E8] font-medium" : "text-stone-700 hover:bg-stone-100"
                }`}
                style={{ width: "calc(100% - 8px)" }}
              >
                {isEditing ? (
                  <input
                    ref={inputRef}
                    value={editValue}
                    onChange={(e) => setEditValue(e.target.value)}
                    onBlur={() => commitEdit(s.id)}
                    onKeyDown={(e) => handleEditKey(e, s.id)}
                    className="flex-1 px-3 py-1.5 text-xs bg-transparent outline-none"
                  />
                ) : (
                  <button
                    onClick={() => onSelect(s.id)}
                    className="flex-1 text-left px-3 py-1.5 text-xs truncate leading-5"
                  >
                    {s.title}
                  </button>
                )}
                {!isEditing && onRename && (
                  <button
                    onClick={(e) => startEdit(s, e)}
                    className="opacity-0 group-hover:opacity-100 pr-2 text-stone-400 hover:text-stone-600 transition-opacity shrink-0"
                    title="Rename"
                  >
                    <svg width="11" height="11" viewBox="0 0 16 16" fill="none">
                      <path d="M11 2l3 3-9 9H2v-3l9-9z" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
                    </svg>
                  </button>
                )}
              </div>
            );
          }

          return (
            <div
              key={s.id ?? i}
              className={`group flex items-start mx-1 rounded transition-colors ${
                isActive ? "bg-blue-50 text-[#1A73E8] font-medium" : "text-stone-700 hover:bg-stone-100"
              }`}
              style={{ width: "calc(100% - 8px)" }}
            >
              {isEditing ? (
                <input
                  ref={inputRef}
                  value={editValue}
                  onChange={(e) => setEditValue(e.target.value)}
                  onBlur={() => commitEdit(s.id)}
                  onKeyDown={(e) => handleEditKey(e, s.id)}
                  className="flex-1 px-3 py-2 text-sm bg-transparent outline-none"
                />
              ) : (
                <button
                  onClick={() => onSelect(s.id)}
                  className="flex-1 text-left px-3 py-2 min-w-0"
                >
                  <div className="truncate leading-5 text-sm">{s.title}</div>
                  <div className="text-xs text-stone-400 mt-0.5">{formatDate(s.updatedAt)}</div>
                </button>
              )}
              {!isEditing && onRename && (
                <button
                  onClick={(e) => startEdit(s, e)}
                  className="opacity-0 group-hover:opacity-100 pt-2.5 pr-2 text-stone-400 hover:text-stone-600 transition-opacity shrink-0"
                  title="Rename"
                >
                  <svg width="11" height="11" viewBox="0 0 16 16" fill="none">
                    <path d="M11 2l3 3-9 9H2v-3l9-9z" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
                  </svg>
                </button>
              )}
            </div>
          );
        })}
      </nav>
    </aside>
  );
}
