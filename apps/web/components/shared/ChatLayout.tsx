"use client";

import { useState, useCallback, useEffect } from "react";
import type { ChatSession, CitationRef } from "@/types/chat";
import { SessionRail } from "./SessionRail";
import { ChatPanel } from "./ChatPanel";
import { DocumentPanel } from "./DocumentPanel";
import { OnboardingChecklist } from "./OnboardingChecklist";
import { useOnboarding } from "@/hooks/useOnboarding";

async function fetchSessions(): Promise<ChatSession[]> {
  try {
    const res = await fetch("/api/sessions");
    return (await res.json()) as ChatSession[];
  } catch {
    return [];
  }
}

async function createSession(title = "New conversation"): Promise<ChatSession | null> {
  try {
    const res = await fetch("/api/sessions", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ title }),
    });
    return (await res.json()) as ChatSession;
  } catch {
    return null;
  }
}

export function ChatLayout() {
  const [sessions, setSessions] = useState<ChatSession[]>([]);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [openDocId, setOpenDocId] = useState<string | null>(null);
  const [scrollToParagraphId, setScrollToParagraphId] = useState<string | null>(null);
  const [docRefreshKey, setDocRefreshKey] = useState(0);
  const onboarding = useOnboarding();

  // On mount: load existing sessions; if none, create one.
  useEffect(() => {
    (async () => {
      const existing = await fetchSessions();
      if (existing.length > 0) {
        setSessions(existing);
        setSessionId(existing[0].id);
      } else {
        const created = await createSession();
        if (created) {
          setSessions([created]);
          setSessionId(created.id);
        }
      }
    })();
  }, []);

  // Auto-complete add_document if documents already exist
  useEffect(() => {
    if (!onboarding.isLoaded || onboarding.state?.completed.add_document) return;
    fetch("/api/documents")
      .then((r) => (r.ok ? r.json() : []))
      .then((docs: unknown[]) => {
        if (docs.length > 0) onboarding.markStepComplete("add_document");
      })
      .catch(() => {});
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [onboarding.isLoaded]);

  function handleCitationClick(ref: CitationRef) {
    setOpenDocId(ref.docId);
    setScrollToParagraphId(ref.paragraphId);
  }

  function handleDocumentUpdated(docId: string) {
    setOpenDocId(docId);
    setScrollToParagraphId(null);
    setDocRefreshKey((k) => k + 1);
  }

  async function handleNewSession() {
    const created = await createSession();
    if (created) {
      setSessions((prev) => [created, ...prev]);
      setSessionId(created.id);
      setOpenDocId(null);
    }
  }

  const handleSessionSelect = useCallback((id: string) => {
    setSessionId(id);
    setOpenDocId(null);
  }, []);

  const handleSessionRenamed = useCallback((id: string, title: string) => {
    setSessions((prev) =>
      prev.map((s) => (s.id === id ? { ...s, title } : s)),
    );
  }, []);

  const handleRenameSession = useCallback(async (id: string, title: string) => {
    handleSessionRenamed(id, title);
    await fetch(`/api/sessions/${id}`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ title }),
    }).catch(() => {});
  }, [handleSessionRenamed]);

  return (
    <div className="flex h-full overflow-hidden bg-stone-50">
      {/* Session rail — hidden on mobile */}
      <div className="hidden md:flex shrink-0">
        <SessionRail
          sessions={sessions}
          currentSessionId={sessionId}
          onSelect={handleSessionSelect}
          onNew={handleNewSession}
          onRename={handleRenameSession}
        />
      </div>

      {/* Chat pane */}
      <div
        className={`flex flex-col min-w-0 border-r border-stone-200 bg-white transition-all duration-200 ${
          openDocId ? "w-1/2" : "flex-1"
        }`}
      >
        {/* Pane header */}
        <div className="flex items-center justify-between px-5 py-3 border-b border-stone-200 shrink-0">
          <div className="flex items-center gap-2">
            <div className="w-6 h-6 rounded-full bg-stone-800 flex items-center justify-center text-white text-xs font-bold">
              K
            </div>
            <span className="text-sm font-semibold text-stone-700">Kaya</span>
          </div>
          {/* Mobile: new session button */}
          <button
            onClick={handleNewSession}
            className="md:hidden p-1.5 rounded hover:bg-stone-100 text-stone-500 transition-colors"
            title="New conversation"
          >
            <svg width="15" height="15" viewBox="0 0 16 16" fill="none">
              <path
                d="M8 3v10M3 8h10"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
              />
            </svg>
          </button>
        </div>

        <ChatPanel
          sessionId={sessionId}
          onCitationClick={handleCitationClick}
          onDocumentUpdated={handleDocumentUpdated}
          onStepComplete={onboarding.markStepComplete}
          onSessionRenamed={handleSessionRenamed}
        />
      </div>

      {/* Document pane */}
      {openDocId && (
        <div className="flex flex-col flex-1 min-w-0 bg-white">
          <DocumentPanel
            docId={openDocId}
            scrollToParagraphId={scrollToParagraphId}
            refreshKey={docRefreshKey}
            onClose={() => setOpenDocId(null)}
          />
        </div>
      )}

      {/* Onboarding checklist — rendered outside the flex layout to avoid reflowing panes */}
      <OnboardingChecklist
        isLoaded={onboarding.isLoaded}
        dismissed={onboarding.state?.dismissed ?? false}
        steps={onboarding.steps}
        demoSeeded={onboarding.state?.demoSeeded ?? false}
        onDismiss={onboarding.dismiss}
        onSeedDemo={onboarding.seedDemo}
        onMarkComplete={onboarding.markStepComplete}
      />
    </div>
  );
}
