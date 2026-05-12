import type { NextRequest } from "next/server";

const API_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:3001";

// Proxies the SSE stream from the Rust backend so the browser doesn't need to
// handle cross-origin streaming. Session cookie forwarding happens here too.
export async function POST(request: NextRequest): Promise<Response> {
  const body = (await request.json()) as { message?: string; sessionId?: string };
  const sessionId = body.sessionId ?? "00000000-0000-0000-0000-000000000000";
  const message = body.message ?? "";

  let upstream: globalThis.Response;
  try {
    upstream = await fetch(`${API_URL}/sessions/${sessionId}/chat`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ message }),
    });
  } catch {
    return Response.json({ error: "backend unreachable" }, { status: 502 });
  }

  if (!upstream.ok || !upstream.body) {
    const text = await upstream.text().catch(() => "");
    return Response.json({ error: text || "upstream error" }, { status: upstream.status });
  }

  return new Response(upstream.body, {
    headers: {
      "Content-Type": "text/event-stream",
      "Cache-Control": "no-cache",
      Connection: "keep-alive",
      "X-Accel-Buffering": "no",
    },
  });
}
