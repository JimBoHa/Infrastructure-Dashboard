"use client";

import { fetchJson, postJson, extractStatus } from "@/lib/http";
import {
  clearAuthToken,
  getAuthToken,
  setAuthToken,
} from "@/lib/authToken";
import React, { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";

type AuthTokenResponse = {
  token: string;
};

type AuthMeResponse = {
  id: string;
  email: string;
  role: string;
  source: string;
  capabilities: string[];
};

type AuthContextValue = {
  ready: boolean;
  token: string | null;
  me: AuthMeResponse | null;
  refresh: () => Promise<void>;
  login: (email: string, password: string) => Promise<void>;
  logout: () => void;
};

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [ready, setReady] = useState(false);
  const [token, setTokenState] = useState<string | null>(null);
  const [me, setMe] = useState<AuthMeResponse | null>(null);

  const refresh = useCallback(async () => {
    const current = getAuthToken();
    if (!current) {
      setMe(null);
      setTokenState(null);
      setReady(true);
      return;
    }
    try {
      const payload = await fetchJson<AuthMeResponse>("/api/auth/me");
      setTokenState(current);
      setMe(payload);
    } catch (error) {
      const status = extractStatus(error);
      if (status === 401) {
        clearAuthToken();
        setTokenState(null);
        setMe(null);
      }
      throw error;
    } finally {
      setReady(true);
    }
  }, []);

  const login = useCallback(
    async (email: string, password: string) => {
      const trimmedEmail = email.trim();
      if (!trimmedEmail || !password.trim()) {
        throw new Error("Email and password are required.");
      }
      const response = await postJson<AuthTokenResponse>("/api/auth/login", {
        email: trimmedEmail,
        password: password.trim(),
      });
      if (!response?.token) {
        throw new Error("Login failed: missing token.");
      }
      setAuthToken(response.token);
      setTokenState(response.token);
      await refresh();
    },
    [refresh],
  );

  const logout = useCallback(() => {
    clearAuthToken();
    setTokenState(null);
    setMe(null);
    setReady(true);
  }, []);

  useEffect(() => {
    setTokenState(getAuthToken());
    void refresh().catch(() => {
      // ignore; the UI will render the login screen when unauthenticated.
    });
  }, [refresh]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const handler = () => {
      setTokenState(getAuthToken());
      void refresh().catch(() => {
        // ignore
      });
    };
    window.addEventListener("storage", handler);
    return () => window.removeEventListener("storage", handler);
  }, [refresh]);

  const value = useMemo<AuthContextValue>(
    () => ({
      ready,
      token,
      me,
      refresh,
      login,
      logout,
    }),
    [ready, token, me, refresh, login, logout],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext);
  if (!ctx) {
    throw new Error("useAuth must be used within <AuthProvider />");
  }
  return ctx;
}
