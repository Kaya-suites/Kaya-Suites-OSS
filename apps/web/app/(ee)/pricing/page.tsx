import Link from "next/link";
import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Pricing — Kaya Suites",
  description: "One cloud plan at $10/month. Or self-host the OSS binary for free.",
};

export default function PricingPage() {
  return (
    <div className="min-h-screen bg-white text-gray-900 font-sans">
      {/* Nav */}
      <nav className="border-b border-gray-100 px-6 py-4 flex items-center justify-between max-w-6xl mx-auto">
        <Link href="/" className="font-semibold text-lg tracking-tight">Kaya Suites</Link>
        <div className="flex items-center gap-6 text-sm">
          <Link href="/pricing" className="text-gray-900 font-medium">Pricing</Link>
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

      <main className="max-w-5xl mx-auto px-6 py-20">
        <div className="text-center mb-16">
          <h1 className="text-4xl font-bold tracking-tight mb-4">Simple pricing</h1>
          <p className="text-lg text-gray-500 max-w-xl mx-auto">
            One cloud plan, everything included. Or download the OSS binary and self-host for free.
          </p>
        </div>

        {/* Plan cards */}
        <div className="grid sm:grid-cols-2 gap-8 mb-20">
          {/* Cloud */}
          <div className="border border-gray-900 rounded-2xl p-8">
            <div className="text-sm font-medium text-gray-500 mb-2">Cloud</div>
            <div className="flex items-baseline gap-1 mb-1">
              <span className="text-6xl font-bold">$10</span>
              <span className="text-gray-500 text-lg">/ month</span>
            </div>
            <p className="text-sm text-gray-500 mb-8">Per workspace. Cancel any time.</p>

            <Link
              href="/billing/subscribe"
              className="block w-full text-center bg-gray-900 text-white py-3 rounded-lg font-semibold hover:bg-gray-700 transition-colors mb-8"
            >
              Get started
            </Link>

            <div className="space-y-3 text-sm text-gray-700">
              <FeatureRow label="Agent invocations" value="50 / month" />
              <FeatureRow label="Documents" value="Unlimited" />
              <FeatureRow label="Storage" value="1 GB" />
              <FeatureRow label="Semantic + full-text search" value="Included" />
              <FeatureRow label="Automatic backups" value="Daily" />
              <FeatureRow label="Multi-device sync" value="Included" />
              <FeatureRow label="Managed Postgres" value="Included" />
              <FeatureRow label="Money-back guarantee" value="30 days" />
              <FeatureRow label="Support" value="Email" />
              <FeatureRow label="Overage invocations" value="$0.10 each" note />
            </div>
          </div>

          {/* OSS */}
          <div className="border border-gray-200 rounded-2xl p-8">
            <div className="text-sm font-medium text-gray-500 mb-2">Open Source</div>
            <div className="flex items-baseline gap-1 mb-1">
              <span className="text-6xl font-bold">Free</span>
            </div>
            <p className="text-sm text-gray-500 mb-8">Forever. Apache 2.0. Bring your own keys.</p>

            <a
              href="https://github.com/kaya-suites/kaya-suites/releases"
              target="_blank"
              rel="noreferrer"
              className="block w-full text-center border border-gray-300 text-gray-700 py-3 rounded-lg font-semibold hover:border-gray-500 transition-colors mb-8"
            >
              Download binary ↗
            </a>

            <div className="space-y-3 text-sm text-gray-700">
              <FeatureRow label="Agent invocations" value="Unlimited" />
              <FeatureRow label="Documents" value="Unlimited" />
              <FeatureRow label="Storage" value="Disk only" />
              <FeatureRow label="Semantic + full-text search" value="Included" />
              <FeatureRow label="Automatic backups" value="Manual" />
              <FeatureRow label="Multi-device sync" value="Not included" faded />
              <FeatureRow label="Managed Postgres" value="Not included" faded />
              <FeatureRow label="Money-back guarantee" value="N/A" faded />
              <FeatureRow label="Support" value="Community" />
              <FeatureRow label="API keys" value="Your own" />
            </div>
          </div>
        </div>

        {/* What counts section */}
        <section className="mb-20">
          <h2 className="text-2xl font-semibold mb-6">What counts as an agent invocation?</h2>
          <div className="prose prose-gray max-w-none text-sm text-gray-600 space-y-3 leading-relaxed">
            <p>
              An agent invocation is one round-trip through the AI editing loop. Specifically, the
              following operations count toward your monthly allotment:
            </p>
            <ul className="list-disc pl-5 space-y-1">
              <li><strong>Edit proposal</strong> — AI detects a stale paragraph and generates a suggested rewrite.</li>
              <li><strong>Document generation</strong> — AI drafts a new document from a prompt or template.</li>
            </ul>
            <p>The following do <strong>not</strong> count:</p>
            <ul className="list-disc pl-5 space-y-1">
              <li>Search queries (semantic or full-text)</li>
              <li>Embedding generation during document import</li>
              <li>Staleness classification (fast classifier, not counted)</li>
              <li>Viewing, editing, or approving documents manually</li>
            </ul>
            <p>
              The 50 included invocations covers most active teams. A typical workspace proposes 10–20
              edits per month. Overages are charged at $0.10 per invocation and appear on your next
              Paddle invoice.
            </p>
          </div>
        </section>

        {/* Cost model */}
        <section className="mb-20">
          <h2 className="text-2xl font-semibold mb-6">AI cost model</h2>
          <p className="text-sm text-gray-500 mb-6">
            Cloud plan AI costs are pooled across all users. Here is the cost structure per operation
            at list prices. Kaya caps your monthly AI spend at $6.00 regardless of usage.
          </p>
          <div className="overflow-hidden border border-gray-200 rounded-xl">
            <table className="w-full text-sm">
              <thead>
                <tr className="bg-gray-50 border-b border-gray-200 text-left text-gray-500">
                  <th className="px-5 py-3">Operation</th>
                  <th className="px-5 py-3">Model</th>
                  <th className="px-5 py-3 text-right">Typical cost</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-100">
                {[
                  { op: "Edit proposal", model: "Claude Opus", cost: "~$0.09" },
                  { op: "Document generation", model: "Claude Opus", cost: "~$0.09" },
                  { op: "Staleness classification", model: "GPT-4o-mini", cost: "~$0.0003" },
                  { op: "Semantic search (import)", model: "text-embedding-3-small", cost: "~$0.000002 / doc" },
                ].map((r) => (
                  <tr key={r.op}>
                    <td className="px-5 py-3 text-gray-800">{r.op}</td>
                    <td className="px-5 py-3 text-gray-500 font-mono text-xs">{r.model}</td>
                    <td className="px-5 py-3 text-right tabular-nums text-gray-700">{r.cost}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="mt-3 text-xs text-gray-400">
            Costs shown are approximate at current API list prices. The $10/month plan is profitable
            at normal usage. At full 50-invocation utilisation the AI cost is ~$4.50.
          </p>
        </section>

        {/* FAQ */}
        <section className="mb-20">
          <h2 className="text-2xl font-semibold mb-8">Pricing FAQ</h2>
          <div className="space-y-8 max-w-2xl">
            {[
              {
                q: "Is there a free trial?",
                a: "No free trial — we offer a 30-day money-back guarantee instead. If you're unsatisfied for any reason within 30 days, email us and we will refund in full, no questions asked.",
              },
              {
                q: "What happens if I hit the 50-invocation limit?",
                a: "Subsequent invocations are billed at $0.10 each and appear on your next invoice. You can view your remaining allotment in the dashboard at any time. We don't hard-block you — you'll just see a note that overages are accruing.",
              },
              {
                q: "Can I change plans?",
                a: "There is currently one cloud plan. We may add team and enterprise tiers in the future. Self-hosted (OSS) is always free.",
              },
              {
                q: "How does the 30-day money-back guarantee work?",
                a: "Email support within 30 days of your first payment. We'll cancel your subscription and issue a full refund via Paddle, typically within 5–10 business days.",
              },
              {
                q: "Do unused invocations roll over?",
                a: "No. The 50 included invocations reset on the first day of each billing period.",
              },
              {
                q: "Do you offer discounts for non-profits or students?",
                a: "Not yet. The OSS self-hosted binary is free forever — it's a good starting point while we figure out a discount programme.",
              },
              {
                q: "Is the cloud plan available globally?",
                a: "Yes. Paddle handles tax collection (VAT, GST) automatically based on your billing address. All prices shown in USD.",
              },
            ].map(({ q, a }) => (
              <div key={q}>
                <h3 className="font-semibold mb-2 text-gray-900">{q}</h3>
                <p className="text-sm text-gray-500 leading-relaxed">{a}</p>
              </div>
            ))}
          </div>
        </section>

        {/* CTA */}
        <div className="text-center border-t border-gray-100 pt-16">
          <h2 className="text-2xl font-semibold mb-4">Ready to keep your docs current?</h2>
          <p className="text-gray-500 mb-8 text-sm">Start today. Cancel or export your data any time.</p>
          <div className="flex flex-col sm:flex-row justify-center gap-4">
            <Link
              href="/billing/subscribe"
              className="bg-gray-900 text-white px-8 py-3.5 rounded-lg font-semibold hover:bg-gray-700 transition-colors"
            >
              Start for $10 / month
            </Link>
            <a
              href="https://github.com/kaya-suites/kaya-suites/releases"
              target="_blank"
              rel="noreferrer"
              className="border border-gray-200 text-gray-700 px-8 py-3.5 rounded-lg font-semibold hover:border-gray-400 transition-colors"
            >
              Download OSS binary ↗
            </a>
          </div>
        </div>
      </main>

      <footer className="border-t border-gray-100 py-10 mt-10">
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

function FeatureRow({
  label,
  value,
  note,
  faded,
}: {
  label: string;
  value: string;
  note?: boolean;
  faded?: boolean;
}) {
  return (
    <div className="flex justify-between items-center py-1 border-b border-gray-50 last:border-0">
      <span className={faded ? "text-gray-400" : "text-gray-600"}>{label}</span>
      <span className={`font-medium tabular-nums ${faded ? "text-gray-400" : "text-gray-900"} ${note ? "text-gray-500 font-normal text-xs" : ""}`}>
        {value}
      </span>
    </div>
  );
}
