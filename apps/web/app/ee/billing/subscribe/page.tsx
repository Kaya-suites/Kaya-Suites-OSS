"use client";

import { useEffect, useRef, useState } from "react";

const API_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:3001";
const PADDLE_CLIENT_TOKEN = process.env.NEXT_PUBLIC_PADDLE_CLIENT_TOKEN ?? "";
const PADDLE_PRICE_ID = process.env.NEXT_PUBLIC_PADDLE_PRICE_ID ?? "";

declare global {
  interface Window {
    Paddle?: {
      Setup(opts: { token: string; eventCallback?: (event: unknown) => void }): void;
      Checkout: {
        open(opts: {
          items: Array<{ priceId: string; quantity: number }>;
          customData?: Record<string, string>;
          successUrl?: string;
        }): void;
      };
    };
  }
}

export default function SubscribePage() {
  const paddleReady = useRef(false);
  const [userId, setUserId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Load Paddle.js once on mount.
  useEffect(() => {
    if (document.querySelector('script[src*="paddle.js"]')) return;
    const script = document.createElement("script");
    script.src = "https://cdn.paddle.com/paddle/v2/paddle.js";
    script.async = true;
    script.onload = () => {
      window.Paddle?.Setup({ token: PADDLE_CLIENT_TOKEN });
      paddleReady.current = true;
    };
    document.head.appendChild(script);
  }, []);

  // Fetch the current user's ID so we can embed it as Paddle custom data.
  useEffect(() => {
    fetch(`${API_URL}/auth/me`, { credentials: "include" })
      .then((r) => (r.ok ? r.json() : null))
      .then((data) => data?.user_id && setUserId(data.user_id))
      .catch(() => null);
  }, []);

  function openCheckout() {
    if (!window.Paddle) {
      setError("Checkout is still loading — please try again in a moment.");
      return;
    }
    if (!userId) {
      setError("You must be signed in to subscribe.");
      return;
    }
    if (!PADDLE_PRICE_ID) {
      setError("Checkout is not configured. Contact support.");
      return;
    }

    setLoading(true);
    setError(null);

    window.Paddle.Checkout.open({
      items: [{ priceId: PADDLE_PRICE_ID, quantity: 1 }],
      customData: { user_id: userId },
      successUrl: `${window.location.origin}/ee/billing/success`,
    });

    // Paddle opens an overlay; reset loading after a short delay so the button
    // doesn't stay disabled if the user closes the overlay without completing.
    setTimeout(() => setLoading(false), 2000);
  }

  return (
    <main className="min-h-screen flex items-center justify-center bg-gray-50 px-4">
      <div className="w-full max-w-md bg-white rounded-xl border border-gray-200 p-10 shadow-sm">
        <h1 className="text-2xl font-semibold text-gray-900 mb-2">
          Kaya Suites
        </h1>
        <p className="text-gray-500 mb-8 leading-relaxed">
          AI-native knowledge base — one plan, everything included.
        </p>

        <div className="rounded-lg border border-gray-100 bg-gray-50 p-6 mb-8">
          <div className="flex items-baseline gap-1 mb-4">
            <span className="text-4xl font-bold text-gray-900">$10</span>
            <span className="text-gray-500">/month</span>
          </div>
          <ul className="space-y-2 text-sm text-gray-600">
            <li>✓ Unlimited documents</li>
            <li>✓ AI-assisted editing &amp; search</li>
            <li>✓ Semantic search across your knowledge base</li>
            <li>✓ Data export at any time</li>
            <li>✓ 30-day money-back guarantee</li>
          </ul>
        </div>

        {error && (
          <p className="text-red-600 text-sm mb-4 rounded-md bg-red-50 border border-red-200 px-4 py-3">
            {error}
          </p>
        )}

        <button
          onClick={openCheckout}
          disabled={loading}
          className="w-full rounded-lg bg-gray-900 text-white py-3 px-6 font-semibold text-base
                     hover:bg-gray-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {loading ? "Opening checkout…" : "Subscribe — $10/month"}
        </button>

        <p className="mt-4 text-center text-xs text-gray-400">
          Secure checkout via Paddle. Cancel any time.
        </p>
      </div>
    </main>
  );
}
