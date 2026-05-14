import type { NextRequest } from "next/server";

const API_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:3001";

const DEMO_TITLE = "Kaya Quickstart Guide";

const DEMO_CONTENT = `# Kaya Quickstart Guide

Welcome to Kaya — your AI-native knowledge base.

## Getting started

Import your Markdown documents and Kaya will keep them fresh automatically. Claude scans for stale facts, version numbers, and outdated processes, then proposes precise diffs for your review.

## How it works

1. **Import** — Upload or paste your Markdown files.
2. **Detect** — Claude Opus identifies facts that may have drifted from reality.
3. **Review** — Each proposed change arrives as a diff. You approve or reject it.

## Current setup

This guide was written for Kaya v0.9 (released January 2026). The recommended Node.js version at the time was 16 LTS. The default embedding model was \`text-embedding-ada-002\`.

## Chat interface

Ask Kaya anything about your documents. It cites the exact paragraph it's drawing from so you can verify the source.

## Next steps

- Ask Kaya to update the version numbers in this guide.
- Review the proposed diff and click **Approve** — that's the core loop.
`;

export async function POST(request: NextRequest): Promise<Response> {
  const cookie = request.headers.get("cookie") ?? "";
  try {
    const res = await fetch(`${API_URL}/documents`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...(cookie && { cookie }),
      },
      body: JSON.stringify({ title: DEMO_TITLE, content: DEMO_CONTENT }),
    });
    const data = await res.json();
    return Response.json(data, { status: res.status });
  } catch {
    return Response.json({ error: "Failed to seed demo document" }, { status: 502 });
  }
}
