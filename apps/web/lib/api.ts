/**
 * Configure the API client with the backend base URL.
 * Import this module once at app startup (e.g. from the root layout or a
 * top-level client component) before calling any API functions.
 */
import { configureClient } from "@kaya/api-client";

export const API_BASE_URL =
  process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:3001";

configureClient({ baseUrl: API_BASE_URL });

export { configureClient };
