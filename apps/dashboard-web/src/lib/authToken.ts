const TOKEN_KEY = "farmdashboard.auth.token";

export function getAuthToken(): string | null {
  if (typeof window !== "undefined") {
    try {
      const sessionToken = window.sessionStorage.getItem(TOKEN_KEY);
      if (sessionToken) {
        return sessionToken;
      }

      // Historical cleanup: older builds persisted tokens in localStorage.
      // The production dashboard requires login per browser session, so we drop
      // any persisted tokens and force a fresh sign-in.
      if (window.localStorage.getItem(TOKEN_KEY)) {
        window.localStorage.removeItem(TOKEN_KEY);
      }

      return process.env.NEXT_PUBLIC_AUTH_TOKEN ?? null;
    } catch {
      return null;
    }
  }
  return process.env.NEXT_PUBLIC_AUTH_TOKEN ?? null;
}

export function setAuthToken(token: string) {
  if (typeof window === "undefined") return;
  try {
    const trimmed = token.trim();
    if (!trimmed) return;
    window.sessionStorage.setItem(TOKEN_KEY, trimmed);
    window.localStorage.removeItem(TOKEN_KEY);
  } catch {
    // ignore
  }
}

export function clearAuthToken() {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.removeItem(TOKEN_KEY);
    window.sessionStorage.removeItem(TOKEN_KEY);
  } catch {
    // ignore
  }
}
