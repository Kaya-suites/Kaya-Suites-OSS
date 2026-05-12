"use client";

import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { Components } from "react-markdown";
import type { ChatMessageData, CitationRef } from "@/types/chat";
import { DiffReview } from "./DiffReview";

type Props = {
  message: ChatMessageData;
  isStreaming?: boolean;
  onCitationClick: (ref: CitationRef) => void;
  onApproveEdit: (editId: string, finalText: string) => Promise<void>;
  onRejectEdit: (editId: string) => void;
};

// Replace [n] patterns with superscript citation chips
function CitationText({
  children,
  citations,
  onCitationClick,
}: {
  children: string;
  citations: CitationRef[];
  onCitationClick: (ref: CitationRef) => void;
}) {
  const parts = children.split(/(\[\d+\])/g);
  return (
    <>
      {parts.map((part, i) => {
        const match = part.match(/^\[(\d+)\]$/);
        if (match) {
          const label = parseInt(match[1], 10);
          const ref = citations.find((c) => c.label === label);
          if (ref) {
            return (
              <sup key={i}>
                <button
                  onClick={() => onCitationClick(ref)}
                  className="inline-flex items-center justify-center w-4 h-4 text-[10px] font-semibold rounded-full bg-[#1A73E8] text-white hover:bg-[#1557B0] transition-colors leading-none ml-0.5 cursor-pointer"
                  title={`Open: ${ref.title}`}
                >
                  {label}
                </button>
              </sup>
            );
          }
        }
        return <span key={i}>{part}</span>;
      })}
    </>
  );
}

export function ChatMessage({
  message,
  isStreaming,
  onCitationClick,
  onApproveEdit,
  onRejectEdit,
}: Props) {
  const isUser = message.role === "user";

  // Build markdown components with citation injection
  const components: Components = {
    p({ children }) {
      const text = typeof children === "string" ? children : null;
      if (text) {
        return (
          <p className="mb-3 last:mb-0">
            <CitationText
              citations={message.citations}
              onCitationClick={onCitationClick}
            >
              {text}
            </CitationText>
          </p>
        );
      }
      // children is an array — map over it
      const nodes = Array.isArray(children) ? children : [children];
      return (
        <p className="mb-3 last:mb-0">
          {nodes.map((child, i) =>
            typeof child === "string" ? (
              <CitationText
                key={i}
                citations={message.citations}
                onCitationClick={onCitationClick}
              >
                {child}
              </CitationText>
            ) : (
              child
            ),
          )}
        </p>
      );
    },
    code({ children, className }) {
      const isBlock = className?.startsWith("language-");
      if (isBlock) {
        return (
          <code className="block bg-stone-100 rounded p-3 text-xs font-mono text-stone-800 overflow-x-auto whitespace-pre">
            {children}
          </code>
        );
      }
      return (
        <code className="bg-stone-100 rounded px-1 py-0.5 text-xs font-mono text-stone-800">
          {children}
        </code>
      );
    },
    pre({ children }) {
      return <pre className="mb-3 last:mb-0">{children}</pre>;
    },
    ul({ children }) {
      return <ul className="list-disc pl-5 mb-3 last:mb-0 space-y-1">{children}</ul>;
    },
    ol({ children }) {
      return <ol className="list-decimal pl-5 mb-3 last:mb-0 space-y-1">{children}</ol>;
    },
    li({ children }) {
      return <li className="text-stone-700">{children}</li>;
    },
    strong({ children }) {
      return <strong className="font-semibold text-stone-900">{children}</strong>;
    },
    a({ href, children }) {
      return (
        <a href={href} className="text-[#1A73E8] underline hover:text-[#1557B0]">
          {children}
        </a>
      );
    },
    table({ children }) {
      return (
        <div className="overflow-x-auto mb-3 last:mb-0">
          <table className="text-sm border-collapse w-full">{children}</table>
        </div>
      );
    },
    th({ children }) {
      return (
        <th className="border border-stone-200 px-3 py-1.5 bg-stone-50 text-left font-semibold text-stone-700 text-xs uppercase tracking-wide">
          {children}
        </th>
      );
    },
    td({ children }) {
      return <td className="border border-stone-200 px-3 py-1.5 text-stone-700">{children}</td>;
    },
  };

  if (isUser) {
    return (
      <div className="flex justify-end mb-4">
        <div className="max-w-[75%] bg-[#1A73E8] text-white rounded-2xl rounded-tr-sm px-4 py-2.5 text-sm leading-relaxed">
          {message.content}
        </div>
      </div>
    );
  }

  return (
    <div className="flex mb-5 gap-3">
      {/* Agent avatar */}
      <div className="shrink-0 w-7 h-7 rounded-full bg-stone-800 flex items-center justify-center text-white text-xs font-semibold mt-0.5">
        K
      </div>

      <div className="flex-1 min-w-0">
        <div className="prose prose-sm max-w-none text-stone-700 [&>*:last-child]:mb-0">
          {message.content ? (
            <ReactMarkdown remarkPlugins={[remarkGfm]} components={components}>
              {message.content}
            </ReactMarkdown>
          ) : isStreaming ? null : null}

          {isStreaming && (
            <span className="inline-block w-2 h-4 ml-1 bg-stone-400 rounded-sm animate-pulse align-text-bottom" />
          )}
        </div>

        {/* Diff review panel */}
        {message.proposedEdit && (
          <DiffReview
            edit={message.proposedEdit}
            onApprove={onApproveEdit}
            onReject={onRejectEdit}
          />
        )}

        {/* Citation list */}
        {message.citations.length > 0 && (
          <div className="mt-3 flex flex-wrap gap-1.5">
            {message.citations.map((c) => (
              <button
                key={c.label}
                onClick={() => onCitationClick(c)}
                className="inline-flex items-center gap-1.5 px-2 py-1 rounded-full border border-stone-200 bg-white text-xs text-stone-600 hover:border-[#1A73E8] hover:text-[#1A73E8] hover:bg-blue-50 transition-colors"
              >
                <span className="inline-flex items-center justify-center w-3.5 h-3.5 rounded-full bg-stone-200 text-[9px] font-bold text-stone-600">
                  {c.label}
                </span>
                {c.title}
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
