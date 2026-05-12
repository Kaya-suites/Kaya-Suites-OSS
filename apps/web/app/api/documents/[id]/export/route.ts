import type { NextRequest } from "next/server";

const API_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:3001";

// Proxy the PDF download so the browser gets it as an attachment regardless
// of CORS configuration on the Rust backend.
export async function GET(
  _req: NextRequest,
  { params }: { params: Promise<{ id: string }> },
): Promise<Response> {
  const { id } = await params;
  try {
    const res = await fetch(`${API_URL}/documents/${id}/export.pdf`);
    if (!res.ok) {
      return Response.json({ error: "not found" }, { status: res.status });
    }
    const blob = await res.blob();
    const disposition = res.headers.get("content-disposition") ?? `attachment; filename="${id}.pdf"`;
    return new Response(blob, {
      headers: {
        "Content-Type": "application/pdf",
        "Content-Disposition": disposition,
      },
    });
  } catch {
    return Response.json({ error: "backend unreachable" }, { status: 502 });
  }
}
