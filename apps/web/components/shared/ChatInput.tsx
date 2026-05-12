"use client";

import { useRef } from "react";

type Props = {
  onSend: (message: string) => void;
  disabled?: boolean;
};

export function ChatInput({ onSend, disabled }: Props) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  function handleKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      submit();
    }
  }

  function submit() {
    const value = textareaRef.current?.value.trim();
    if (!value || disabled) return;
    onSend(value);
    if (textareaRef.current) textareaRef.current.value = "";
    // Reset height
    if (textareaRef.current) textareaRef.current.style.height = "auto";
  }

  function handleInput() {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
  }

  return (
    <div className="border-t border-stone-200 bg-white px-4 py-3">
      <div className="flex items-end gap-2 rounded-xl border border-stone-300 bg-white px-3 py-2 focus-within:border-[#1A73E8] focus-within:ring-1 focus-within:ring-[#1A73E8] transition-all">
        <textarea
          ref={textareaRef}
          onKeyDown={handleKeyDown}
          onInput={handleInput}
          disabled={disabled}
          placeholder="Ask about your documents… (Enter to send, Shift+Enter for newline)"
          rows={1}
          className="flex-1 resize-none border-none outline-none bg-transparent text-sm text-stone-800 placeholder:text-stone-400 leading-relaxed py-0.5 disabled:opacity-50"
        />
        <button
          onClick={submit}
          disabled={disabled}
          className="shrink-0 mb-0.5 p-1.5 rounded-lg bg-[#1A73E8] text-white hover:bg-[#1557B0] disabled:opacity-50 transition-colors"
          title="Send"
        >
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
            <path
              d="M2 8l12-6-5 6 5 6-12-6z"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinejoin="round"
              fill="none"
            />
          </svg>
        </button>
      </div>
      <p className="mt-1.5 text-center text-[10px] text-stone-400">
        Kaya AI · Responses may contain errors. Always verify cited sources.
      </p>
    </div>
  );
}
