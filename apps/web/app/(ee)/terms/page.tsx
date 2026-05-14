import Link from "next/link";
import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Terms of Service — Kaya Suites",
  description: "Terms governing use of the Kaya Suites cloud service.",
};

const LAST_UPDATED = "May 13, 2026";

export default function TermsPage() {
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
        <h1 className="text-3xl font-bold mb-2">Terms of Service</h1>
        <p className="text-sm text-gray-400 mb-12">Last updated: {LAST_UPDATED}</p>

        <div className="prose prose-gray prose-sm max-w-none space-y-10 text-gray-700 leading-relaxed">
          <Section title="1. Acceptance">
            <p>
              By accessing or using the Kaya Suites cloud service ("Service") you agree to these Terms.
              If you are using the Service on behalf of an organisation, you represent that you have
              authority to bind that organisation.
            </p>
          </Section>

          <Section title="2. Description of the Service">
            <p>
              Kaya Suites provides an AI-assisted knowledge base: document storage, semantic search,
              and an AI editing loop that detects stale content and proposes edits for human review.
              The Service is offered under a subscription (see §6).
            </p>
            <p>
              A separate open-source binary ("OSS") is distributed under the Apache 2.0 licence.
              These Terms do not apply to self-hosted OSS deployments; those are governed solely by
              the Apache 2.0 licence.
            </p>
          </Section>

          <Section title="3. Accounts">
            <p>
              You must provide a valid email address. You are responsible for all activity that
              occurs under your account. Notify us immediately at{" "}
              <a href="mailto:support@kaya-suites.com" className="underline">
                support@kaya-suites.com
              </a>{" "}
              if you suspect unauthorised access.
            </p>
          </Section>

          <Section title="4. Acceptable use">
            <p>You agree not to:</p>
            <ul>
              <li>Upload content you do not have the right to share.</li>
              <li>Attempt to circumvent usage limits or billing controls.</li>
              <li>Use the Service to generate content that is unlawful, harmful, or violates third-party rights.</li>
              <li>Reverse engineer or attempt to extract the source code of BSL-licensed components.</li>
              <li>Resell or sublicense the Service without written permission.</li>
            </ul>
          </Section>

          <Section title="5. Your content">
            <p>
              You retain ownership of all documents and content you create in the Service. By using
              the Service you grant us a limited, worldwide, royalty-free licence to store, process,
              and transmit your content solely to provide the Service.
            </p>
            <p>
              Your content is processed by Anthropic and OpenAI APIs under zero-data-retention
              agreements. We do not use your content to train AI models.
            </p>
          </Section>

          <Section title="6. Subscription and billing">
            <p>
              The cloud plan is $10 USD per workspace per month, billed via Paddle. Subscription
              renews automatically unless cancelled.
            </p>
            <p>
              Overage invocations (beyond 50/month included) are charged at $0.10 per invocation,
              billed on the next invoice. You can view accrued overages in your dashboard.
            </p>
            <p>
              <strong>30-day money-back guarantee:</strong> If you are unsatisfied within 30 days of
              your first payment, email us for a full refund, no questions asked.
            </p>
          </Section>

          <Section title="7. Cancellation">
            <p>
              You may cancel your subscription at any time from your dashboard. Access continues
              until the end of the current billing period. No refunds for partial periods (except
              under the 30-day guarantee in §6).
            </p>
          </Section>

          <Section title="8. Service availability">
            <p>
              We aim for high availability but do not guarantee any specific uptime SLA on the
              current plan. We will notify registered users of planned maintenance.
            </p>
          </Section>

          <Section title="9. Intellectual property">
            <p>
              The Kaya Suites name, logo, and cloud service are proprietary. The OSS core is
              available under Apache 2.0. Enterprise ("ee/") components are under BSL 1.1 —
              source-available for reading but not for running in production without a licence.
            </p>
          </Section>

          <Section title="10. Limitation of liability">
            <p>
              To the maximum extent permitted by law, Kaya Suites shall not be liable for indirect,
              incidental, special, or consequential damages arising from your use of the Service.
              Our total liability for direct damages shall not exceed the fees paid by you in the
              12 months preceding the claim.
            </p>
          </Section>

          <Section title="11. Disclaimer of warranties">
            <p>
              THE SERVICE IS PROVIDED "AS IS" WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
              INCLUDING WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE, OR
              NON-INFRINGEMENT.
            </p>
          </Section>

          <Section title="12. Changes to the Service or Terms">
            <p>
              We may modify these Terms or the Service at any time. We will provide at least 14
              days notice via email for material changes. Continued use after the effective date
              constitutes acceptance.
            </p>
          </Section>

          <Section title="13. Governing law">
            <p>
              These Terms are governed by the laws of the State of California, USA, without regard
              to conflict of law principles.
            </p>
          </Section>

          <Section title="14. Contact">
            <p>
              <a href="mailto:support@kaya-suites.com" className="underline">
                support@kaya-suites.com
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
