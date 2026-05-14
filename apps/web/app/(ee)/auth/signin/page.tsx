"use client";

import Link from "next/link";
import { useState } from "react";

const API_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:3001";

type State = "idle" | "loading" | "sent" | "error";

export default function SignInPage() {
  const [email, setEmail] = useState("");
  const [state, setState] = useState<State>("idle");

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!email.trim()) return;
    setState("loading");

    const r = await fetch(`${API_URL}/auth/request-link`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ email: email.trim() }),
    });

    if (r.ok) {
      setState("sent");
    } else {
      setState("error");
    }
  }

  return (
    <main className="min-h-screen bg-gray-50 flex items-center justify-center px-4">
      <div className="w-full max-w-sm">
        <div className="mb-8 text-center">
          <Link href="/" className="font-semibold text-lg tracking-tight text-gray-900">
            Kaya Suites
          </Link>
        </div>

        <div className="bg-white rounded-2xl border border-gray-200 p-8">
          {state === "sent" ? (
            <div className="text-center space-y-3">
              <div className="text-3xl">✉️</div>
              <h1 className="font-semibold text-gray-900">Check your email</h1>
              <p className="text-sm text-gray-500 leading-relaxed">
                We sent a sign-in link to <strong>{email}</strong>. It expires in 15 minutes.
              </p>
              <button
                onClick={() => { setEmail(""); setState("idle"); }}
                className="text-sm text-gray-400 underline hover:text-gray-700 mt-2"
              >
                Use a different email
              </button>
            </div>
          ) : (
            <>
              <h1 className="font-semibold text-gray-900 mb-1">Sign in</h1>
              <p className="text-sm text-gray-500 mb-6">
                We&apos;ll email you a magic link — no password needed.
              </p>

              <form onSubmit={handleSubmit} className="space-y-4">
                <div>
                  <label htmlFor="email" className="block text-xs text-gray-500 mb-1.5">
                    Email address
                  </label>
                  <input
                    id="email"
                    type="email"
                    value={email}
                    onChange={(e) => setEmail(e.target.value)}
                    placeholder="you@example.com"
                    required
                    autoFocus
                    className="w-full border border-gray-200 rounded-lg px-3 py-2.5 text-sm focus:outline-none focus:ring-2 focus:ring-gray-900 focus:border-transparent"
                  />
                </div>

                {state === "error" && (
                  <p className="text-xs text-red-600">
                    Something went wrong. Please try again.
                  </p>
                )}

                <button
                  type="submit"
                  disabled={state === "loading" || !email.trim()}
                  className="w-full bg-gray-900 text-white py-2.5 rounded-lg text-sm font-semibold hover:bg-gray-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                >
                  {state === "loading" ? "Sending…" : "Send sign-in link"}
                </button>
              </form>
            </>
          )}
        </div>

        <p className="text-center text-xs text-gray-400 mt-6">
          New here?{" "}
          <Link href="/billing/subscribe" className="underline hover:text-gray-700">
            Subscribe to get started
          </Link>
        </p>
      </div>
    </main>
  );
}
