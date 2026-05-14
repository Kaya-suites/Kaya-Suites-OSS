"use client";

import { useEffect, useRef, useState } from "react";
import type { OnboardingStep } from "@/hooks/useOnboarding";

type Step = { id: OnboardingStep; label: string; done: boolean };

type Props = {
  isLoaded: boolean;
  dismissed: boolean;
  steps: Step[];
  demoSeeded: boolean;
  onDismiss: () => void;
  onSeedDemo: () => Promise<void>;
  onMarkComplete: (step: OnboardingStep) => void;
};

export function OnboardingChecklist({
  isLoaded,
  dismissed,
  steps,
  onDismiss,
  onSeedDemo,
  onMarkComplete,
}: Props) {
  const [expanded, setExpanded] = useState(true);
  const [celebrating, setCelebrating] = useState(false);
  const [seeding, setSeeding] = useState(false);
  const prevAha = useRef(false);

  const ahaStep = steps.find((s) => s.id === "approve_first_diff");
  const ahaComplete = ahaStep?.done ?? false;

  useEffect(() => {
    if (ahaComplete && !prevAha.current) {
      prevAha.current = true;
      setCelebrating(true);
      const t = setTimeout(() => {
        setCelebrating(false);
        setExpanded(false);
      }, 2000);
      return () => clearTimeout(t);
    }
  }, [ahaComplete]);

  if (!isLoaded || dismissed) return null;

  const doneCount = steps.filter((s) => s.done).length;
  const totalCount = steps.length;
  const progressPct = Math.round((doneCount / totalCount) * 100);
  const allDone = doneCount === totalCount;

  async function handleSeedDemo() {
    setSeeding(true);
    try {
      await onSeedDemo();
    } finally {
      setSeeding(false);
    }
  }

  if (!expanded) {
    return (
      <button
        onClick={() => setExpanded(true)}
        className="fixed bottom-4 left-4 z-50 flex items-center gap-2 bg-white border border-stone-200 rounded-full px-3 py-1.5 shadow-sm text-sm text-stone-700 hover:bg-stone-50 transition-colors"
      >
        <svg viewBox="0 0 20 20" className="w-4 h-4 -rotate-90 flex-shrink-0">
          <circle cx="10" cy="10" r="8" fill="none" stroke="#e7e5e4" strokeWidth="2.5" />
          <circle
            cx="10"
            cy="10"
            r="8"
            fill="none"
            stroke={allDone ? "#16a34a" : "#44403c"}
            strokeWidth="2.5"
            strokeDasharray={`${(progressPct / 100) * 50.3} 50.3`}
            strokeLinecap="round"
          />
        </svg>
        <span className="font-medium">{doneCount} / {totalCount} steps complete</span>
      </button>
    );
  }

  return (
    <div className="fixed bottom-4 left-4 z-50 w-[280px]">
      <div
        className={`bg-white border rounded-xl shadow-md overflow-hidden ${
          celebrating ? "border-green-300" : "border-stone-200"
        }`}
      >
        {/* Header */}
        <div
          className={`flex items-center justify-between px-4 py-3 border-b ${
            celebrating ? "bg-green-50 border-green-200" : "border-stone-100"
          }`}
        >
          <div className="flex items-center gap-2">
            <div className="w-5 h-5 rounded-full bg-stone-800 flex items-center justify-center text-white text-[10px] font-bold flex-shrink-0">
              K
            </div>
            <span className="text-sm font-semibold text-stone-800">
              {celebrating ? "Great work — keep going!" : "Get started with Kaya"}
            </span>
          </div>
          <button
            onClick={onDismiss}
            className="text-stone-400 hover:text-stone-600 transition-colors flex-shrink-0"
            title="Dismiss"
          >
            <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
              <path
                d="M1 1l10 10M11 1L1 11"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
              />
            </svg>
          </button>
        </div>

        {/* Progress bar */}
        <div className="h-0.5 bg-stone-100">
          <div
            className={`h-full transition-all duration-500 ${
              allDone ? "bg-green-500" : "bg-stone-700"
            }`}
            style={{ width: `${progressPct}%` }}
          />
        </div>

        {/* Steps */}
        <div className="px-4 py-3 space-y-3">
          {steps.map((step) => (
            <div key={step.id}>
              <div className="flex items-start gap-2.5">
                <div
                  className={`mt-0.5 w-4 h-4 rounded-full border flex-shrink-0 flex items-center justify-center transition-colors ${
                    step.done
                      ? "bg-stone-800 border-stone-800"
                      : "border-stone-300"
                  }`}
                >
                  {step.done && (
                    <svg width="8" height="6" viewBox="0 0 8 6" fill="none">
                      <path
                        d="M1 3l2 2 4-4"
                        stroke="white"
                        strokeWidth="1.5"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      />
                    </svg>
                  )}
                </div>
                <span
                  className={`text-sm leading-tight ${
                    step.done ? "line-through text-stone-400" : "text-stone-700"
                  }`}
                >
                  {step.label}
                </span>
              </div>

              {/* add_document CTAs */}
              {step.id === "add_document" && !step.done && (
                <div className="ml-[26px] mt-2 flex gap-2">
                  <button
                    onClick={handleSeedDemo}
                    disabled={seeding}
                    className="text-xs px-2.5 py-1 bg-stone-800 text-white rounded-md hover:bg-stone-700 disabled:opacity-50 transition-colors"
                  >
                    {seeding ? "Loading…" : "Try a demo doc"}
                  </button>
                  <a
                    href="/documents"
                    className="text-xs px-2.5 py-1 border border-stone-300 text-stone-700 rounded-md hover:bg-stone-50 transition-colors"
                  >
                    Import my own
                  </a>
                </div>
              )}

              {/* set_api_key instruction (OSS) */}
              {step.id === "set_api_key" && !step.done && (
                <div className="ml-[26px] mt-2 space-y-1.5">
                  <p className="text-xs text-stone-500">
                    Add{" "}
                    <code className="bg-stone-100 px-1 rounded font-mono">
                      ANTHROPIC_API_KEY
                    </code>{" "}
                    to your <code className="bg-stone-100 px-1 rounded font-mono">.env</code> file.
                  </p>
                  <button
                    onClick={() => onMarkComplete("set_api_key")}
                    className="text-xs text-stone-500 underline hover:text-stone-700 transition-colors"
                  >
                    I&apos;ve set it
                  </button>
                </div>
              )}
            </div>
          ))}
        </div>

        {/* Footer */}
        <div className="px-4 pb-3">
          <button
            onClick={() => setExpanded(false)}
            className="text-xs text-stone-400 hover:text-stone-600 transition-colors"
          >
            Minimize
          </button>
        </div>
      </div>
    </div>
  );
}
