"use client";

import { useAuth } from "@/components/AuthProvider";
import LoadingState from "@/components/LoadingState";
import { fetchJson, postJson } from "@/lib/http";
import { getDevLoginCredentials, shouldOfferDevLogin } from "@/lib/devLogin";

import { Card } from "@/components/ui/card";
import InlineBanner from "@/components/InlineBanner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useRouter, useSearchParams } from "next/navigation";
import { useEffect, useMemo, useState } from "react";

type AuthBootstrap = { has_users: boolean };

const DEFAULT_ADMIN_CAPABILITIES = [
  "nodes.view",
  "sensors.view",
  "outputs.view",
  "schedules.view",
  "metrics.view",
  "backups.view",
  "setup.credentials.view",
  "config.write",
  "users.manage",
  "schedules.write",
  "outputs.command",
  "alerts.view",
  "alerts.ack",
  "analytics.view",
];

export default function LoginClient() {
  const { me, login } = useAuth();
  const router = useRouter();
  const searchParams = useSearchParams();
  const next = searchParams.get("next") || "/overview";

  const devLoginEnabled = useMemo(() => shouldOfferDevLogin(), []);
  const { email: devLoginEmail, password: devLoginPassword } = useMemo(
    () => getDevLoginCredentials(),
    [],
  );
  const devLoginConfigured = Boolean(devLoginEmail && devLoginPassword);

  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [devBusy, setDevBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [usersLoading, setUsersLoading] = useState(true);
  const [hasUsers, setHasUsers] = useState<boolean | null>(null);

  const [name, setName] = useState("");

  useEffect(() => {
    if (me) {
      router.replace(next);
    }
  }, [me, next, router]);

  const isLoopbackHost = useMemo(() => {
    if (typeof window === "undefined") return false;
    const host = window.location.hostname;
    return host === "localhost" || host === "127.0.0.1" || host === "::1";
  }, []);

  useEffect(() => {
    let cancelled = false;
    setUsersLoading(true);
    void fetchJson<AuthBootstrap>("/api/auth/bootstrap")
      .then((status) => {
        if (cancelled) return;
        setHasUsers(Boolean(status?.has_users));
      })
      .catch(() => {
        if (cancelled) return;
        setHasUsers(true);
      })
      .finally(() => {
        if (cancelled) return;
        setUsersLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const canBootstrapAdmin = useMemo(
    () => hasUsers === false && isLoopbackHost,
    [hasUsers, isLoopbackHost],
  );

  const title = useMemo(() => {
    if (hasUsers) return "Sign in";
    if (canBootstrapAdmin) return "Create admin user";
    return "Setup required";
  }, [canBootstrapAdmin, hasUsers]);

  const submitLogin = async (event: React.FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await login(email, password);
      router.replace(next);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Login failed.");
    } finally {
      setBusy(false);
    }
  };

  const submitDevLogin = async () => {
    setDevBusy(true);
    setError(null);
    try {
      if (!devLoginEmail || !devLoginPassword) {
        throw new Error(
          "Dev login requires NEXT_PUBLIC_DEV_LOGIN_EMAIL and NEXT_PUBLIC_DEV_LOGIN_PASSWORD.",
        );
      }
      await login(devLoginEmail, devLoginPassword);
      router.replace(next);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Dev login failed.");
    } finally {
      setDevBusy(false);
    }
  };

  const submitCreateAdmin = async (event: React.FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      if (!canBootstrapAdmin) {
        throw new Error(
          "For security, first-user creation is only allowed from localhost. Use Setup Center or open the dashboard on the controller at http://127.0.0.1.",
        );
      }
      const trimmedName = name.trim();
      const trimmedEmail = email.trim();
      const trimmedPassword = password.trim();
      if (!trimmedName || !trimmedEmail || !trimmedPassword) {
        throw new Error("Name, email, and password are required.");
      }

      await postJson("/api/users", {
        name: trimmedName,
        email: trimmedEmail,
        password: trimmedPassword,
        role: "admin",
        capabilities: DEFAULT_ADMIN_CAPABILITIES,
      });

      await login(trimmedEmail, trimmedPassword);
      router.replace(next);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unable to create admin user.");
    } finally {
      setBusy(false);
    }
  };

  if (usersLoading) {
    return (
 <div className="flex min-h-screen items-center justify-center bg-card-inset px-4 py-10">
        <div className="w-full max-w-md">
          <LoadingState label="Preparing sign-in…" />
        </div>
      </div>
    );
  }

  return (
 <div className="flex min-h-screen items-center justify-center bg-card-inset px-4 py-10">
      <div className="w-full max-w-md space-y-6">
        <header className="space-y-1 text-center">
          <div className="mx-auto inline-flex size-12 items-center justify-center rounded-2xl bg-indigo-600 text-base font-semibold text-white">
            FD
          </div>
          <h1 className="text-2xl font-semibold text-card-foreground">
            Farm Dashboard
          </h1>
 <p className="text-sm text-muted-foreground">{title} to continue.</p>
        </header>

        <Card className="p-6">
          <form
            className="space-y-4"
            onSubmit={hasUsers ? submitLogin : canBootstrapAdmin ? submitCreateAdmin : (event) => event.preventDefault()}
          >
            {!hasUsers && !canBootstrapAdmin ? (
              <InlineBanner tone="warning" className="px-3 py-2 text-sm">
                No users exist yet. For safety, first-user creation is disabled over the network.
                Use Setup Center on the controller, or open this dashboard locally at{" "}
                <code className="px-1">http://127.0.0.1</code>.
              </InlineBanner>
            ) : null}

            {!hasUsers && canBootstrapAdmin ? (
              <div>
 <label className="mb-2 block text-sm font-medium text-foreground">
                  Name
                </label>
                <Input
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                  autoComplete="name"
                />
              </div>
            ) : null}

            <div>
 <label className="mb-2 block text-sm font-medium text-foreground">
                Email
              </label>
              <Input
                value={email}
                onChange={(event) => setEmail(event.target.value)}
                type="email"
                autoComplete="email"
                placeholder="admin@farmdashboard.local"
              />
            </div>

            <div>
 <label className="mb-2 block text-sm font-medium text-foreground">
                Password
              </label>
              <Input
                value={password}
                onChange={(event) => setPassword(event.target.value)}
                type="password"
                autoComplete={hasUsers ? "current-password" : canBootstrapAdmin ? "new-password" : "current-password"}
              />
            </div>

            {error ? (
              <InlineBanner tone="danger" className="px-3 py-2 text-sm">
                {error}
              </InlineBanner>
            ) : null}

            <Button
              type="submit"
              variant="primary"
              fullWidth
              loading={busy}
              disabled={!hasUsers && !canBootstrapAdmin}
            >
              {hasUsers ? "Sign in" : canBootstrapAdmin ? "Create admin & sign in" : "Sign in"}
            </Button>
          </form>

          {hasUsers && devLoginEnabled && devLoginConfigured ? (
            <div className="mt-4 border-t border-border pt-4">
              <Button
                type="button"
                variant="dashed"
                fullWidth
                loading={devBusy}
                onClick={submitDevLogin}
              >
                Login as Dev
              </Button>
            </div>
          ) : null}
        </Card>

 <p className="text-center text-xs text-muted-foreground">
          This controller is on your local network. If you lose access, restart the stack from Setup Center.
        </p>
      </div>
    </div>
  );
}
