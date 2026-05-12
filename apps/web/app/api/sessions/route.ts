import type { NextRequest } from "next/server";

const API_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:3001";

export async function GET(): Promise<Response> {
  try {
    const res = await fetch(`${API_URL}/sessions`);
    const data = await res.json();
    return Response.json(data, { status: res.status });
  } catch {
    return Response.json([], { status: 200 }); // empty list when backend is down
  }
}

export async function POST(request: NextRequest): Promise<Response> {
  const body = await request.json();
  try {
    const res = await fetch(`${API_URL}/sessions`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    const data = await res.json();
    return Response.json(data, { status: res.status });
  } catch {
    return Response.json({ error: "backend unreachable" }, { status: 502 });
  }
}
