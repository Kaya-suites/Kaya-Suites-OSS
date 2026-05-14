import Link from "next/link";
import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Privacy Policy — Kaya Suites",
  description: "How Kaya Suites collects, uses, and protects your data.",
};

const LAST_UPDATED = "May 13, 2026";

export default function PrivacyPage() {
  return (
    <div className="min-h-screen bg-white text-gray-900 font-sans">
      <nav className="border-b border-gray-100 px-6 py-4 flex items-center justify-between max-w-6xl mx-auto">
        <Link href="/" className="font-semibold text-lg tracking-tight">Kaya Suites</Link>
        <Link
          href="/auth/signin"
          className="bg-gray-900 text-white px-4 py-2 rounded-lg text-sm font-medium hover:bg-gray-700 transition-colors"
        >
          Sign in
        </Link>
      </nav>

      <main className="max-w-2xl mx-auto px-6 py-20">
        <h1 className="text-3xl font-bold mb-2">Privacy Policy</h1>
        <p className="text-sm text-gray-400 mb-12">Last updated: {LAST_UPDATED}</p>

        <div className="prose prose-gray prose-sm max-w-none space-y-10 text-gray-700 leading-relaxed">
          <Section title="1. What we collect">
            <p>When you create an account we collect your email address. When you use the cloud service we collect:</p>
            <ul>
              <li>Document content you create or import.</li>
              <li>Usage events (operation type, token counts, model used, timestamp) for billing and rate-limit enforcement.</li>
              <li>Session tokens stored in an HTTP-only cookie.</li>
              <li>Subscription and billing information processed by Paddle (we receive a Paddle customer ID and subscription ID; we do not store card numbers).</li>
            </ul>
          </Section>

          <Section title="2. How we use your data">
            <ul>
              <li>To provide the Kaya Suites service: document storage, AI editing loop, search.</li>
              <li>To calculate and enforce per-user usage limits and spend caps.</li>
              <li>To send transactional emails (magic-link sign-in, usage alerts) via Resend.</li>
              <li>To process subscription payments via Paddle.</li>
            </ul>
            <p>We do not use your data for advertising and we do not sell it to third parties.</p>
          </Section>

          <Section title="3. AI and your document content">
            <p>
              Edit proposals and document generation are processed by Anthropic (Claude) and OpenAI APIs.
              Both providers are configured in <strong>zero-data-retention mode</strong>: your content is
              not logged by the provider and is not used to train their models. Staleness classification
              and embedding generation are also sent to these APIs under the same policy.
            </p>
            <p>
              If you use the self-hosted OSS binary, your content goes directly from your server to
              whichever API you configure. We never see it.
            </p>
          </Section>

          <Section title="4. Data storage and security">
            <p>
              Cloud data is stored in a managed Postgres instance (Neon). Connections are encrypted with
              TLS. Backups are taken daily and retained for 7 days. Session cookies are HTTP-only,
              Secure, and SameSite=Lax.
            </p>
          </Section>

          <Section title="5. Data retention and deletion">
            <p>
              You may export all documents and usage history from your dashboard at any time. To delete
              your account, email{" "}
              <a href="mailto:privacy@kaya-suites.com" className="underline">
                privacy@kaya-suites.com
              </a>
              . Account deletion permanently removes all stored documents, usage records, and personal
              data within 30 days.
            </p>
            <p>
              After subscription cancellation, read access continues until the end of your billing period.
              Data is retained for 30 days after that to allow re-activation, then permanently deleted.
            </p>
          </Section>

          <Section title="6. Cookies">
            <p>
              We use a single session cookie (<code>kaya_session</code>) for authentication. No analytics
              cookies. No third-party tracking.
            </p>
          </Section>

          <Section title="7. Third-party services">
            <ul>
              <li><strong>Paddle</strong> — subscription billing and tax collection.</li>
              <li><strong>Resend</strong> — transactional email delivery.</li>
              <li><strong>Anthropic</strong> — Claude API for edit proposals and generation.</li>
              <li><strong>OpenAI</strong> — GPT-4o-mini for classification; text-embedding-3-small for search.</li>
              <li><strong>Neon</strong> — managed Postgres hosting.</li>
            </ul>
          </Section>

          <Section title="8. Your rights">
            <p>
              Depending on your jurisdiction you may have rights to access, rectify, or erase your
              personal data, or to object to processing. To exercise these rights, email{" "}
              <a href="mailto:privacy@kaya-suites.com" className="underline">
                privacy@kaya-suites.com
              </a>
              .
            </p>
          </Section>

          <Section title="9. Changes to this policy">
            <p>
              We will post updates here and email registered users for material changes. Continued use
              after the effective date constitutes acceptance.
            </p>
          </Section>

          <Section title="10. Contact">
            <p>
              Questions?{" "}
              <a href="mailto:privacy@kaya-suites.com" className="underline">
                privacy@kaya-suites.com
              </a>
            </p>
          </Section>
        </div>
      </main>

      <footer className="border-t border-gray-100 py-10">
        <div className="max-w-6xl mx-auto px-6 flex flex-col sm:flex-row items-center justify-between gap-4 text-sm text-gray-400">
          <span>© {new Date().getFullYear()} Kaya Suites</span>
          <div className="flex gap-6">
            <Link href="/pricing" className="hover:text-gray-700 transition-colors">Pricing</Link>
            <Link href="/privacy" className="hover:text-gray-700 transition-colors">Privacy</Link>
            <Link href="/terms" className="hover:text-gray-700 transition-colors">Terms</Link>
          </div>
        </div>
      </footer>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section>
      <h2 className="text-base font-semibold text-gray-900 mb-3">{title}</h2>
      <div className="space-y-3">{children}</div>
    </section>
  );
}
