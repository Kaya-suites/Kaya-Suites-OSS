"use client";

import { useEffect, useRef, useState } from "react";
import type {
  ChatMessageData,
  CitationRef,
  Role,
  SSEEvent,
} from "@/types/chat";
import { ChatMessage } from "./ChatMessage";
import { ChatInput } from "./ChatInput";

type Props = {
  sessionId: string | null;
  onCitationClick: (ref: CitationRef) => void;
  onDocumentUpdated: (docId: string) => void;
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

export function ChatPanel({ sessionId, onCitationClick, onDocumentUpdated }: Props) {
  const [messages, setMessages] = useState<ChatMessageData[]>([WELCOME]);
  const [streamingId, setStreamingId] = useState<string | null>(null);
  const bottomRef = useRef<HTMLDivElement>(null);

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

    setMessages((prev) => [...prev, userMsg, assistantMsg]);
    setStreamingId(assistantId);

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
              setMessages((prev) =>
                prev.map((m) =>
                  m.id === assistantId
                    ? {
                        ...m,
                        proposedEdit: {
                          id: event.editId,
                          docId: event.docId,
                          paragraphId: event.paragraphId,
                          original: event.original,
                          proposed: event.proposed,
                          status: "pending",
                        },
                      }
                    : m,
                ),
              );
            } else if (event.type === "Done") {
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

  async function approveEdit(editId: string, finalText: string) {
    // Find the edit to get the docId
    const msg = messages.find((m) => m.proposedEdit?.id === editId);
    if (!msg?.proposedEdit) return;

    const res = await fetch(`/api/edits/${editId}/approve`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ proposed: finalText }),
    });

    if (!res.ok) throw new Error("approve failed");

    setMessages((prev) =>
      prev.map((m) =>
        m.id === msg.id && m.proposedEdit
          ? { ...m, proposedEdit: { ...m.proposedEdit, status: "approved" } }
          : m,
      ),
    );

    onDocumentUpdated(msg.proposedEdit.docId);
  }

  function rejectEdit(editId: string) {
    setMessages((prev) =>
      prev.map((m) =>
        m.proposedEdit?.id === editId && m.proposedEdit
          ? { ...m, proposedEdit: { ...m.proposedEdit, status: "rejected" } }
          : m,
      ),
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
          />
        ))}
        <div ref={bottomRef} />
      </div>

      {/* Input */}
      <ChatInput onSend={sendMessage} disabled={streamingId !== null || sessionId === null} />
    </div>
  );
}
