import Link from "next/link";

export default function BillingSuccessPage() {
  return (
    <main className="min-h-screen flex items-center justify-center bg-gray-50 px-4">
      <div className="w-full max-w-md bg-white rounded-xl border border-gray-200 p-10 shadow-sm text-center">
        <div className="text-5xl mb-6">🎉</div>
        <h1 className="text-2xl font-semibold text-gray-900 mb-3">
          Welcome to Kaya Suites
        </h1>
        <p className="text-gray-500 leading-relaxed mb-8">
          Your subscription is active. We&apos;ve sent a confirmation email.
          You&apos;re covered by our 30-day money-back guarantee.
        </p>

        <Link
          href="/"
          className="inline-block rounded-lg bg-gray-900 text-white py-3 px-8 font-semibold
                     hover:bg-gray-700 transition-colors"
        >
          Go to dashboard →
        </Link>

        <p className="mt-8 text-xs text-gray-400">
          Need help?{" "}
          <a href="mailto:support@kaya.io" className="underline">
            Contact support
          </a>
        </p>
      </div>
    </main>
  );
}
