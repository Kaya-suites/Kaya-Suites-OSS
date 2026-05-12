const API_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:3001";

export async function GET(): Promise<Response> {
  try {
    const res = await fetch(`${API_URL}/documents`);
    const data = await res.json();
    return Response.json(data, { status: res.status });
  } catch {
    return Response.json([], { status: 200 });
  }
}
