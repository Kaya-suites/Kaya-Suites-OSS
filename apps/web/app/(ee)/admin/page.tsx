"use client";

import { useEffect, useState } from "react";

const API_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:3001";

interface UserStats {
  user_id: string;
  email: string;
  monthly_cost_usd: number;
  agent_invocations: number;
}

interface AdminStats {
  aggregate_daily_spend_usd: number;
  aggregate_monthly_spend_usd: number;
  circuit_breaker_active: boolean;
  top_users: UserStats[];
  total_users: number;
  active_subscriptions: number;
}

function fmt(usd: number) {
  return `$${usd.toFixed(4)}`;
}

export default function AdminPage() {
  const [stats, setStats] = useState<AdminStats | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [resetting, setResetting] = useState(false);

  async function fetchStats() {
    const r = await fetch(`${API_URL}/admin/stats`, { credentials: "include" });
    if (r.status === 401) { setError("Not authenticated."); return; }
    if (r.status === 403) { setError("Access denied — admin only."); return; }
    if (!r.ok) { setError("Failed to load stats."); return; }
    setStats(await r.json());
  }

  async function resetCircuitBreaker() {
    setResetting(true);
    const r = await fetch(`${API_URL}/admin/circuit-breaker/reset`, {
      method: "POST",
      credentials: "include",
    });
    setResetting(false);
    if (r.ok) fetchStats();
  }

  useEffect(() => { fetchStats(); }, []);

  if (error) {
    return (
      <main className="min-h-screen flex items-center justify-center bg-gray-50">
        <p className="text-red-600">{error}</p>
      </main>
    );
  }

  if (!stats) {
    return (
      <main className="min-h-screen flex items-center justify-center bg-gray-50">
        <p className="text-gray-400">Loading…</p>
      </main>
    );
  }

  return (
    <main className="min-h-screen bg-gray-50 p-8">
      <div className="max-w-5xl mx-auto space-y-8">
        <h1 className="text-2xl font-semibold text-gray-900">Founder Dashboard</h1>

        {/* Circuit breaker alert */}
        {stats.circuit_breaker_active && (
          <div className="rounded-lg bg-red-50 border border-red-200 p-4 flex items-center justify-between">
            <p className="text-red-700 font-medium">
              ⚠ Circuit breaker OPEN — new agent invocations are blocked.
            </p>
            <button
              onClick={resetCircuitBreaker}
              disabled={resetting}
              className="ml-4 text-sm bg-red-700 text-white rounded px-3 py-1.5 hover:bg-red-600 disabled:opacity-50"
            >
              {resetting ? "Resetting…" : "Reset"}
            </button>
          </div>
        )}

        {/* KPI row */}
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <Kpi label="Daily spend" value={fmt(stats.aggregate_daily_spend_usd)} />
          <Kpi label="Monthly spend" value={fmt(stats.aggregate_monthly_spend_usd)} />
          <Kpi label="Total users" value={String(stats.total_users)} />
          <Kpi label="Active subs" value={String(stats.active_subscriptions)} />
        </div>

        {/* Top users table */}
        <div className="bg-white rounded-xl border border-gray-200 overflow-hidden">
          <div className="px-6 py-4 border-b border-gray-100">
            <h2 className="font-semibold text-gray-800">Top users by monthly spend</h2>
          </div>
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-gray-500 border-b border-gray-100">
                <th className="px-6 py-3">Email</th>
                <th className="px-6 py-3 text-right">Spend (MTD)</th>
                <th className="px-6 py-3 text-right">Invocations</th>
              </tr>
            </thead>
            <tbody>
              {stats.top_users.map((u) => (
                <tr key={u.user_id} className="border-b border-gray-50 last:border-0">
                  <td className="px-6 py-3 text-gray-700 font-mono text-xs">{u.email}</td>
                  <td className="px-6 py-3 text-right tabular-nums">{fmt(u.monthly_cost_usd)}</td>
                  <td className="px-6 py-3 text-right tabular-nums">{u.agent_invocations}</td>
                </tr>
              ))}
              {stats.top_users.length === 0 && (
                <tr>
                  <td colSpan={3} className="px-6 py-6 text-center text-gray-400">
                    No usage this period.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>

        <p className="text-xs text-gray-400 text-right">
          Refresh to update ·{" "}
          <button onClick={fetchStats} className="underline">Reload now</button>
        </p>
      </div>
    </main>
  );
}

function Kpi({ label, value }: { label: string; value: string }) {
  return (
    <div className="bg-white rounded-xl border border-gray-200 p-5">
      <p className="text-xs text-gray-500 mb-1">{label}</p>
      <p className="text-xl font-semibold text-gray-900 tabular-nums">{value}</p>
    </div>
  );
}
