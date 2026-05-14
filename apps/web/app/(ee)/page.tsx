import Link from "next/link";
import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Kaya Suites — Docs that keep themselves current",
  description:
    "AI-native knowledge base that detects stale content, proposes edits, and learns from your team's decisions.",
};

export default function LandingPage() {
  return (
    <div className="min-h-screen bg-white text-gray-900 font-sans">
      {/* ── Nav ───────────────────────────────────────────────────── */}
      <nav className="border-b border-gray-100 px-6 py-4 flex items-center justify-between max-w-6xl mx-auto">
        <span className="font-semibold text-lg tracking-tight">Kaya Suites</span>
        <div className="flex items-center gap-6 text-sm">
          <Link href="/pricing" className="text-gray-500 hover:text-gray-900 transition-colors">
            Pricing
          </Link>
          <a
            href="https://github.com/kaya-suites/kaya-suites"
            className="text-gray-500 hover:text-gray-900 transition-colors"
            target="_blank"
            rel="noreferrer"
          >
            GitHub
          </a>
          <Link
            href="/auth/signin"
            className="bg-gray-900 text-white px-4 py-2 rounded-lg text-sm font-medium hover:bg-gray-700 transition-colors"
          >
            Sign in
          </Link>
        </div>
      </nav>

      {/* ── Hero ──────────────────────────────────────────────────── */}
      <section className="max-w-4xl mx-auto px-6 pt-24 pb-20 text-center">
        <h1 className="text-5xl sm:text-6xl font-bold tracking-tight leading-tight mb-6">
          Docs that keep themselves current.
        </h1>
        <p className="text-xl text-gray-500 leading-relaxed max-w-2xl mx-auto mb-10">
          Kaya Suites is an AI-native knowledge base. It detects stale content,
          proposes precise edits, and shows you a diff before anything changes —
          so your documentation stays accurate without becoming a second job.
        </p>
        <div className="flex flex-col sm:flex-row items-center justify-center gap-4">
          <Link
            href="/billing/subscribe"
            className="bg-gray-900 text-white px-8 py-3.5 rounded-lg font-semibold text-base hover:bg-gray-700 transition-colors"
          >
            Start for $10 / month
          </Link>
          <a
            href="https://github.com/kaya-suites/kaya-suites/releases"
            className="border border-gray-200 text-gray-700 px-8 py-3.5 rounded-lg font-semibold text-base hover:border-gray-400 transition-colors"
            target="_blank"
            rel="noreferrer"
          >
            Download OSS binary ↗
          </a>
        </div>
        <p className="mt-4 text-sm text-gray-400">
          30-day money-back guarantee. No free trial. OSS self-hosted is free forever.
        </p>
      </section>

      {/* ── How it works ──────────────────────────────────────────── */}
      <section className="bg-gray-50 border-y border-gray-100 py-20">
        <div className="max-w-5xl mx-auto px-6">
          <h2 className="text-3xl font-semibold text-center mb-14">How it works</h2>
          <div className="grid sm:grid-cols-3 gap-10">
            {[
              {
                step: "01",
                title: "Import your docs",
                body: "Connect your Markdown files or paste content directly. Kaya indexes every paragraph for semantic search.",
              },
              {
                step: "02",
                title: "AI detects drift",
                body: "When facts become stale — a version number, a changed API, an outdated process — Kaya surfaces the paragraph and explains why.",
              },
              {
                step: "03",
                title: "You approve, not rubber-stamp",
                body: "Every edit arrives as a diff. Accept, reject, or refine. Nothing merges without your explicit approval.",
              },
            ].map(({ step, title, body }) => (
              <div key={step}>
                <div className="text-xs font-mono text-gray-400 mb-2">{step}</div>
                <h3 className="font-semibold text-lg mb-2">{title}</h3>
                <p className="text-gray-500 leading-relaxed text-sm">{body}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* ── Pricing ───────────────────────────────────────────────── */}
      <section className="max-w-4xl mx-auto px-6 py-20 text-center">
        <h2 className="text-3xl font-semibold mb-4">Simple pricing</h2>
        <p className="text-gray-500 mb-12">
          One plan. Everything included. Or self-host for free.
        </p>
        <div className="flex flex-col sm:flex-row gap-6 justify-center">
          {/* Cloud */}
          <div className="flex-1 max-w-sm border border-gray-900 rounded-2xl p-8 text-left">
            <div className="text-sm font-medium text-gray-500 mb-4">Cloud</div>
            <div className="flex items-baseline gap-1 mb-6">
              <span className="text-5xl font-bold">$10</span>
              <span className="text-gray-500">/ month</span>
            </div>
            <ul className="space-y-2.5 text-sm text-gray-600 mb-8">
              {[
                "50 agent invocations / month",
                "Unlimited documents",
                "1 GB storage",
                "Semantic + full-text search",
                "30-day money-back guarantee",
                "Automatic backups",
              ].map((f) => (
                <li key={f} className="flex items-start gap-2">
                  <span className="text-gray-900 mt-px">✓</span> {f}
                </li>
              ))}
            </ul>
            <Link
              href="/billing/subscribe"
              className="w-full block text-center bg-gray-900 text-white py-3 rounded-lg font-semibold hover:bg-gray-700 transition-colors"
            >
              Get started
            </Link>
          </div>

          {/* OSS */}
          <div className="flex-1 max-w-sm border border-gray-200 rounded-2xl p-8 text-left">
            <div className="text-sm font-medium text-gray-500 mb-4">Open Source</div>
            <div className="flex items-baseline gap-1 mb-6">
              <span className="text-5xl font-bold">Free</span>
            </div>
            <ul className="space-y-2.5 text-sm text-gray-600 mb-8">
              {[
                "Single binary, zero dependencies",
                "Bring your own API keys",
                "Local SQLite storage",
                "Full source on GitHub (Apache 2.0)",
                "Community support",
                "No usage limits",
              ].map((f) => (
                <li key={f} className="flex items-start gap-2">
                  <span className="text-gray-500 mt-px">✓</span> {f}
                </li>
              ))}
            </ul>
            <a
              href="https://github.com/kaya-suites/kaya-suites/releases"
              target="_blank"
              rel="noreferrer"
              className="w-full block text-center border border-gray-200 text-gray-700 py-3 rounded-lg font-semibold hover:border-gray-400 transition-colors"
            >
              Download binary ↗
            </a>
          </div>
        </div>
        <p className="mt-6 text-sm text-gray-400">
          <Link href="/pricing" className="underline">Full pricing details →</Link>
        </p>
      </section>

      {/* ── FAQ ───────────────────────────────────────────────────── */}
      <section className="bg-gray-50 border-t border-gray-100 py-20">
        <div className="max-w-2xl mx-auto px-6">
          <h2 className="text-3xl font-semibold text-center mb-12">FAQ</h2>
          <div className="space-y-8">
            {[
              {
                q: "What counts as an agent invocation?",
                a: "One agent invocation is one chat message that triggers the AI loop — typically an edit proposal or document generation. Search, retrieval, and embeddings are not counted. The 50/month included allotment covers active teams; overages are billed at cost.",
              },
              {
                q: "Is there a free trial?",
                a: "No free trial — but there is a 30-day money-back guarantee with no questions asked. You can also self-host the OSS binary indefinitely with your own API keys.",
              },
              {
                q: "How does the OSS version differ from cloud?",
                a: "The OSS binary (Apache 2.0) includes the full document management, AI editing loop, and search. Cloud adds multi-device sync, automatic backups, managed Postgres, and the billing/auth layer. The BSL-licensed features are never in the public repo.",
              },
              {
                q: "What AI models does Kaya use?",
                a: "Claude Opus for edit proposals and document generation; GPT-4o-mini for retrieval classification and stale detection; text-embedding-3-small for semantic search. You can bring your own keys in the OSS version.",
              },
              {
                q: "What happens to my data if I cancel?",
                a: "You can export all documents and chat history at any time from your dashboard. After cancellation, read access continues until the end of your billing period. Account deletion permanently removes all data.",
              },
              {
                q: "Is my data used to train AI models?",
                a: "No. We use Anthropic and OpenAI APIs in zero-data-retention mode. Your document content is never used for training.",
              },
            ].map(({ q, a }) => (
              <div key={q}>
                <h3 className="font-semibold mb-2">{q}</h3>
                <p className="text-gray-500 leading-relaxed text-sm">{a}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* ── Footer ────────────────────────────────────────────────── */}
      <footer className="border-t border-gray-100 py-10">
        <div className="max-w-6xl mx-auto px-6 flex flex-col sm:flex-row items-center justify-between gap-4 text-sm text-gray-400">
          <span>© {new Date().getFullYear()} Kaya Suites</span>
          <div className="flex gap-6">
            <Link href="/pricing" className="hover:text-gray-700 transition-colors">Pricing</Link>
            <Link href="/privacy" className="hover:text-gray-700 transition-colors">Privacy</Link>
            <Link href="/terms" className="hover:text-gray-700 transition-colors">Terms</Link>
            <a href="https://github.com/kaya-suites/kaya-suites" target="_blank" rel="noreferrer" className="hover:text-gray-700 transition-colors">GitHub</a>
          </div>
        </div>
      </footer>
    </div>
  );
}
