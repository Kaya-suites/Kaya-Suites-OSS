"use client";

import { useCallback, useEffect, useState } from "react";

export type OnboardingStep =
  | "add_document"
  | "send_first_message"
  | "approve_first_diff"
  | "subscribe"
  | "set_api_key";

export type OnboardingTrack = "cloud" | "oss";

export interface OnboardingState {
  track: OnboardingTrack;
  dismissed: boolean;
  completed: Partial<Record<OnboardingStep, true>>;
  demoSeeded: boolean;
}

const STORAGE_KEY = "kaya_onboarding_v1";

const CLOUD_STEPS: { id: OnboardingStep; label: string }[] = [
  { id: "add_document", label: "Add your first document" },
  { id: "send_first_message", label: "Ask Kaya a question" },
  { id: "approve_first_diff", label: "Approve an AI-proposed edit" },
  { id: "subscribe", label: "Subscribe to keep going" },
];

const OSS_STEPS: { id: OnboardingStep; label: string }[] = [
  { id: "set_api_key", label: "Set your Anthropic API key" },
  { id: "add_document", label: "Add your first document" },
  { id: "approve_first_diff", label: "Approve an AI-proposed edit" },
];

function deriveTrack(): OnboardingTrack {
  return process.env.NEXT_PUBLIC_KAYA_BUILD === "oss" ? "oss" : "cloud";
}

function loadState(): OnboardingState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return JSON.parse(raw) as OnboardingState;
  } catch {}
  return { track: deriveTrack(), dismissed: false, completed: {}, demoSeeded: false };
}

function persist(s: OnboardingState): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(s));
  } catch {}
}

export function useOnboarding() {
  const [state, setState] = useState<OnboardingState | null>(null);

  // Defer to client — avoids SSR hydration mismatch
  useEffect(() => {
    setState(loadState());
  }, []);

  // Cloud: auto-complete subscribe if billing already active
  useEffect(() => {
    if (!state || state.track !== "cloud" || state.completed.subscribe) return;
    fetch("/api/billing/status")
      .then((r) => (r.ok ? r.json() : null))
      .then((data: { status?: string } | null) => {
        if (data?.status === "active") {
          setState((prev) => {
            if (!prev) return prev;
            const next = { ...prev, completed: { ...prev.completed, subscribe: true as const } };
            persist(next);
            return next;
          });
        }
      })
      .catch(() => {});
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state?.track]);

  const markStepComplete = useCallback((step: OnboardingStep) => {
    setState((prev) => {
      if (!prev || prev.completed[step]) return prev;
      const next = { ...prev, completed: { ...prev.completed, [step]: true as const } };
      persist(next);
      return next;
    });
  }, []);

  const dismiss = useCallback(() => {
    setState((prev) => {
      if (!prev) return prev;
      const next = { ...prev, dismissed: true };
      persist(next);
      return next;
    });
  }, []);

  const seedDemo = useCallback(async () => {
    const res = await fetch("/api/documents/seed-demo", { method: "POST" });
    if (!res.ok) throw new Error("demo seed failed");
    setState((prev) => {
      if (!prev) return prev;
      const next = {
        ...prev,
        demoSeeded: true,
        completed: { ...prev.completed, add_document: true as const },
      };
      persist(next);
      return next;
    });
  }, []);

  const trackSteps = state?.track === "oss" ? OSS_STEPS : CLOUD_STEPS;
  const steps = trackSteps.map((s) => ({ ...s, done: !!(state?.completed[s.id]) }));

  return { state, isLoaded: state !== null, steps, markStepComplete, dismiss, seedDemo };
}
