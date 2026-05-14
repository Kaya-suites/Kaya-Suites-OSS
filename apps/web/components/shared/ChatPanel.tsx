"use client";

import { useEffect, useRef, useState } from "react";
import type {
  ChatMessageData,
  CitationRef,
  Role,
  SSEEvent,
  ProposedEdit,
  ProposedDelete,
} from "@/types/chat";
import type { OnboardingStep } from "@/hooks/useOnboarding";
import { ChatMessage } from "./ChatMessage";
import { ChatInput } from "./ChatInput";

type Props = {
  sessionId: string | null;
  onCitationClick: (ref: CitationRef) => void;
  onDocumentUpdated: (docId: string) => void;
  onStepComplete?: (step: OnboardingStep) => void;
};

function randomId(): string {
  return crypto.randomUUID();
}

function parseSSELine(line: string): SSEEvent | null {
  if (!line.startsWith("data: ")) return null;
  try {
    return JSON.parse(line.slice(6)) as SSEEvent;
  } catch {
    return null;
  }
}

const WELCOME: ChatMessageData = {
  id: "welcome",
  role: "assistant",
  content:
    "Hello! I'm Kaya, your AI knowledge assistant. Ask me anything about your documents — I'll cite sources as I go. You can also ask me to **update** or **edit** a document and I'll propose a change for your review.",
  citations: [],
  timestamp: Date.now(),
};

export function ChatPanel({ sessionId, onCitationClick, onDocumentUpdated, onStepComplete }: Props) {
  const [messages, setMessages] = useState<ChatMessageData[]>([WELCOME]);
  const [streamingId, setStreamingId] = useState<string | null>(null);
  // Tracks current text for each pending edit card so "Approve All" reads the right value
  const [pendingEditTexts, setPendingEditTexts] = useState<Record<string, string>>({});
  const bottomRef = useRef<HTMLDivElement>(null);
  // Buffers edits/deletes during a streaming turn; flushed all at once on Done
  const editBufferRef = useRef<ProposedEdit[]>([]);
  const deleteBufferRef = useRef<ProposedDelete[]>([]);

  // Scroll to bottom on new content
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  // Load message history when session changes
  useEffect(() => {
    setStreamingId(null);
    if (!sessionId) {
      setMessages([WELCOME]);
      return;
    }
    let cancelled = false;
    fetch(`/api/sessions/${sessionId}/messages`)
      .then((res) => (res.ok ? res.json() : []))
      .then(
        (
          data: Array<{
            id: string;
            role: string;
            content: string;
            citations: CitationRef[];
            createdAt: number;
          }>,
        ) => {
          if (cancelled) return;
          if (data.length === 0) {
            setMessages([WELCOME]);
          } else {
            setMessages(
              data.map((m) => ({
                id: m.id,
                role: m.role as Role,
                content: m.content,
                citations: m.citations ?? [],
                timestamp: m.createdAt,
              })),
            );
          }
        },
      )
      .catch(() => {
        if (!cancelled) setMessages([WELCOME]);
      });
    return () => {
      cancelled = true;
    };
  }, [sessionId]);

  async function sendMessage(content: string) {
    if (streamingId || sessionId === null) return;
    const sid: string = sessionId;

    const userMsg: ChatMessageData = {
      id: randomId(),
      role: "user",
      content,
      citations: [],
      timestamp: Date.now(),
    };

    const assistantId = randomId();
    const assistantMsg: ChatMessageData = {
      id: assistantId,
      role: "assistant",
      content: "",
      citations: [],
      timestamp: Date.now(),
    };

    const isFirstMessage = messages.filter((m) => m.role === "user").length === 0;
    setMessages((prev) => [...prev, userMsg, assistantMsg]);
    setStreamingId(assistantId);
    editBufferRef.current = [];
    deleteBufferRef.current = [];
    if (isFirstMessage) onStepComplete?.("send_first_message");

    try {
      const res = await fetch("/api/chat", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ sessionId: sid, message: content }),
      });

      if (!res.ok) {
        const data = await res.json().catch(() => null) as { error?: string } | null;
        const msg = data?.error ?? `Server error ${res.status}`;
        setMessages((prev) =>
          prev.map((m) =>
            m.id === assistantId ? { ...m, content: `**Error:** ${msg}` } : m,
          ),
        );
        setStreamingId(null);
        return;
      }
      if (!res.body) return;
      const reader = res.body.getReader();
      const decoder = new TextDecoder();
      let buffer = "";

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        buffer += decoder.decode(value, { stream: true });

        const blocks = buffer.split("\n\n");
        buffer = blocks.pop() ?? "";

        for (const block of blocks) {
          for (const line of block.split("\n")) {
            const event = parseSSELine(line);
            if (!event) continue;

            if (event.type === "TextChunk") {
              setMessages((prev) =>
                prev.map((m) =>
                  m.id === assistantId
                    ? { ...m, content: m.content + event.content }
                    : m,
                ),
              );
            } else if (event.type === "CitationFound") {
              setMessages((prev) =>
                prev.map((m) =>
                  m.id === assistantId
                    ? {
                        ...m,
                        citations: [
                          ...m.citations,
                          {
                            label: event.label,
                            docId: event.docId,
                            paragraphId: event.paragraphId,
                            title: event.title,
                          },
                        ],
                      }
                    : m,
                ),
              );
            } else if (event.type === "ProposedEditEmitted") {
              // Buffer the edit — it will be flushed to the message on Done
              editBufferRef.current.push({
                id: event.editId,
                docId: event.docId ?? "",
                paragraphId: event.paragraphId,
                original: event.original,
                proposed: event.proposed,
                status: "pending",
              });
            } else if (event.type === "ProposedDeleteEmitted") {
              deleteBufferRef.current.push({
                id: event.editId,
                docId: event.docId,
                docTitle: event.docTitle,
                status: "pending",
              });
            } else if (event.type === "Done") {
              // Flush all buffered edits/deletes at once so cards appear together
              const edits = editBufferRef.current;
              const deletes = deleteBufferRef.current;
              editBufferRef.current = [];
              deleteBufferRef.current = [];

              if (edits.length > 0 || deletes.length > 0) {
                const newTexts: Record<string, string> = {};
                for (const e of edits) newTexts[e.id] = e.proposed;
                setPendingEditTexts((prev) => ({ ...prev, ...newTexts }));

                setMessages((prev) =>
                  prev.map((m) =>
                    m.id === assistantId
                      ? {
                          ...m,
                          ...(edits.length > 0 ? { proposedEdits: edits } : {}),
                          ...(deletes.length > 0 ? { proposedDeletes: deletes } : {}),
                        }
                      : m,
                  ),
                );
              }

              setStreamingId(null);
            } else if (event.type === "Error") {
              setMessages((prev) =>
                prev.map((m) =>
                  m.id === assistantId
                    ? { ...m, content: `Error: ${event.message}` }
                    : m,
                ),
              );
              setStreamingId(null);
            }
          }
        }
      }
    } catch (err) {
      setMessages((prev) =>
        prev.map((m) =>
          m.id === assistantId
            ? { ...m, content: "Connection error. Please try again." }
            : m,
        ),
      );
    } finally {
      setStreamingId(null);
    }
  }

  function onEditTextChange(editId: string, text: string) {
    setPendingEditTexts((prev) => ({ ...prev, [editId]: text }));
  }

  async function approveEdit(editId: string, finalText: string) {
    const msg = messages.find((m) => m.proposedEdits?.some((e) => e.id === editId));
    const edit = msg?.proposedEdits?.find((e) => e.id === editId);
    if (!msg || !edit) return;

    const res = await fetch(`/api/edits/${editId}/approve`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ proposed: finalText }),
    });

    if (!res.ok) throw new Error("approve failed");

    setMessages((prev) =>
      prev.map((m) =>
        m.id === msg.id && m.proposedEdits
          ? {
              ...m,
              proposedEdits: m.proposedEdits.map((e) =>
                e.id === editId ? { ...e, status: "approved" } : e,
              ),
            }
          : m,
      ),
    );

    onStepComplete?.("approve_first_diff");
    onDocumentUpdated(edit.docId);
  }

  function rejectEdit(editId: string) {
    setMessages((prev) =>
      prev.map((m) =>
        m.proposedEdits?.some((e) => e.id === editId)
          ? {
              ...m,
              proposedEdits: m.proposedEdits!.map((e) =>
                e.id === editId ? { ...e, status: "rejected" } : e,
              ),
            }
          : m,
      ),
    );
  }

  async function approveDelete(editId: string) {
    const res = await fetch(`/api/edits/${editId}/approve`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({}),
    });
    if (!res.ok) throw new Error("approve failed");
    setMessages((prev) =>
      prev.map((m) =>
        m.proposedDeletes?.some((d) => d.id === editId)
          ? {
              ...m,
              proposedDeletes: m.proposedDeletes!.map((d) =>
                d.id === editId ? { ...d, status: "approved" } : d,
              ),
            }
          : m,
      ),
    );
  }

  function rejectDelete(editId: string) {
    setMessages((prev) =>
      prev.map((m) =>
        m.proposedDeletes?.some((d) => d.id === editId)
          ? {
              ...m,
              proposedDeletes: m.proposedDeletes!.map((d) =>
                d.id === editId ? { ...d, status: "rejected" } : d,
              ),
            }
          : m,
      ),
    );
  }

  async function approveAll(messageId: string) {
    const msg = messages.find((m) => m.id === messageId);
    if (!msg) return;

    const pendingEdits = msg.proposedEdits?.filter((e) => e.status === "pending") ?? [];
    const pendingDeletes = msg.proposedDeletes?.filter((d) => d.status === "pending") ?? [];

    await Promise.all([
      ...pendingEdits.map((e) =>
        approveEdit(e.id, pendingEditTexts[e.id] ?? e.proposed),
      ),
      ...pendingDeletes.map((d) => approveDelete(d.id)),
    ]);
  }

  function rejectAll(messageId: string) {
    setMessages((prev) =>
      prev.map((m) => {
        if (m.id !== messageId) return m;
        return {
          ...m,
          proposedEdits: m.proposedEdits?.map((e) =>
            e.status === "pending" ? { ...e, status: "rejected" } : e,
          ),
          proposedDeletes: m.proposedDeletes?.map((d) =>
            d.status === "pending" ? { ...d, status: "rejected" } : d,
          ),
        };
      }),
    );
  }

  return (
    <div className="flex flex-col h-full min-h-0">
      {/* Messages */}
      <div className="flex-1 overflow-y-auto px-5 py-5 min-h-0">
        {messages.map((msg) => (
          <ChatMessage
            key={msg.id}
            message={msg}
            isStreaming={msg.id === streamingId}
            onCitationClick={onCitationClick}
            onApproveEdit={approveEdit}
            onRejectEdit={rejectEdit}
            onApproveDelete={approveDelete}
            onRejectDelete={rejectDelete}
            onEditTextChange={onEditTextChange}
            editTexts={pendingEditTexts}
            onApproveAll={approveAll}
            onRejectAll={rejectAll}
          />
        ))}
        <div ref={bottomRef} />
      </div>

      {/* Input */}
      <ChatInput onSend={sendMessage} disabled={streamingId !== null || sessionId === null} />
    </div>
  );
}
