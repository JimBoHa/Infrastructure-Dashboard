import { clearAuthToken, getAuthToken } from "@/lib/authToken";

export const API_BASE = "";

const normalizeBase = (value: string) => value.replace(/\/$/, "");

const apiBaseNormalized = normalizeBase(API_BASE);

function applyAuthHeader(url: string, headers?: HeadersInit): Headers {
  const merged = new Headers(headers);
  if (merged.has("Authorization")) return merged;
  const token = getAuthToken();
  if (!token) return merged;
  if (url.startsWith("http")) {
    if (!apiBaseNormalized) return merged;
    if (!normalizeBase(url).startsWith(apiBaseNormalized)) return merged;
  }
  merged.set("Authorization", `Bearer ${token}`);
  return merged;
}

export const apiUrl = (path: string) =>
  path.startsWith("http") ? path : apiBaseNormalized ? `${apiBaseNormalized}${path}` : path;

export async function fetchJson<T = unknown>(
  path: string,
  init?: RequestInit,
): Promise<T> {
  const url = apiUrl(path);
  const response = await fetch(url, {
    next: { revalidate: 0 },
    ...init,
    headers: applyAuthHeader(url, init?.headers),
  });
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
    }
    const text = await response.text();
    throw new Error(
      `Request failed (${response.status}): ${text || response.statusText}`,
    );
  }
  return (await response.json()) as T;
}

export async function fetchResponse(path: string, init?: RequestInit): Promise<Response> {
  const url = apiUrl(path);
  const response = await fetch(url, {
    next: { revalidate: 0 },
    ...init,
    headers: applyAuthHeader(url, init?.headers),
  });
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
    }
    const text = await response.text();
    throw new Error(`Request failed (${response.status}): ${text || response.statusText}`);
  }
  return response;
}

export const fetcher = <T>(url: string) => fetchJson<T>(url);

export async function fetchBinary(
  path: string,
  init?: RequestInit,
): Promise<ArrayBuffer> {
  const url = apiUrl(path);
  const response = await fetch(url, {
    next: { revalidate: 0 },
    ...init,
    headers: applyAuthHeader(url, init?.headers),
  });
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
    }
    const text = await response.text();
    throw new Error(
      `Request failed (${response.status}): ${text || response.statusText}`,
    );
  }
  return response.arrayBuffer();
}

async function mutateJson<T = unknown>(
  method: "POST" | "PUT" | "PATCH" | "DELETE",
  path: string,
  body?: unknown,
  init?: RequestInit,
): Promise<T> {
  const url = apiUrl(path);
  const headers = applyAuthHeader(url, init?.headers);
  if (!headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }

  const response = await fetch(url, {
    method,
    headers,
    body: body !== undefined ? JSON.stringify(body) : undefined,
    ...init,
  });
  if (!response.ok) {
    if (response.status === 401) {
      clearAuthToken();
    }
    const text = await response.text();
    throw new Error(
      `Request failed (${response.status}): ${text || response.statusText}`,
    );
  }
  if (response.status === 204) {
    return undefined as T;
  }
  return (await response.json()) as T;
}

export const postJson = <T = unknown>(path: string, body?: unknown, init?: RequestInit) =>
  mutateJson<T>("POST", path, body, init);

export const putJson = <T = unknown>(path: string, body?: unknown, init?: RequestInit) =>
  mutateJson<T>("PUT", path, body, init);

export const deleteJson = <T = unknown>(path: string, init?: RequestInit) =>
  mutateJson<T>("DELETE", path, undefined, init);

const HTTP_STATUS_REGEX = /Request failed \((\d{3})/;

export function extractStatus(error: unknown): number | null {
  if (error instanceof Error) {
    const match = HTTP_STATUS_REGEX.exec(error.message);
    if (match) {
      return Number(match[1]);
    }
  }
  return null;
}
