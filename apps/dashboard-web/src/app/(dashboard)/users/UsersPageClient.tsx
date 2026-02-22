"use client";

import { useId, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { queryKeys, useUsersQuery } from "@/lib/queries";
import LoadingState from "@/components/LoadingState";
import ErrorState from "@/components/ErrorState";
import InlineBanner from "@/components/InlineBanner";
import PageHeaderCard from "@/components/PageHeaderCard";
import CollapsibleCard from "@/components/CollapsibleCard";
import { formatDistanceToNow } from "date-fns";
import { deleteJson, postJson, putJson } from "@/lib/api";
import type { DemoUser } from "@/types/dashboard";
import { useAuth } from "@/components/AuthProvider";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import NodeButton from "@/features/nodes/components/NodeButton";

const KNOWN_CAPABILITIES = [
  "nodes.view",
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
  "sensors.view",
];

const ROLE_CAPABILITIES: Record<string, string[]> = {
  admin: [
    "nodes.view",
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
    "sensors.view",
  ],
  operator: [
    "nodes.view",
    "outputs.view",
    "schedules.view",
    "metrics.view",
    "sensors.view",
    "schedules.write",
    "outputs.command",
    "alerts.view",
    "alerts.ack",
    "analytics.view",
  ],
  view: [
    "nodes.view",
    "outputs.view",
    "schedules.view",
    "metrics.view",
    "sensors.view",
    "alerts.view",
    "analytics.view",
  ],
};

export default function UsersPageClient() {
  const queryClient = useQueryClient();
  const { me: currentUser, refresh: refreshAuth } = useAuth();
  const canManageUsers = Boolean(currentUser?.capabilities?.includes("users.manage"));
  const { data: users = [], error, isLoading } = useUsersQuery({ enabled: canManageUsers });
  const [showModal, setShowModal] = useState(false);
  const [passwordTarget, setPasswordTarget] = useState<DemoUser | null>(null);
  const [message, setMessage] = useState<{ type: "success" | "error"; text: string } | null>(null);
  const [capDrafts, setCapDrafts] = useState<Record<string, string>>({});

  if (canManageUsers && isLoading) return <LoadingState label="Loading users…" />;
  if (error) {
    return <ErrorState message={error instanceof Error ? error.message : "Failed to load users."} />;
  }

  const removeUser = async (user: DemoUser) => {
    if (!canManageUsers) {
      setMessage({ type: "error", text: "Insufficient permissions: users.manage is required to manage users." });
      return;
    }
    try {
      await deleteJson(`/api/users/${user.id}`);
      setMessage({ type: "success", text: `Removed ${user.name}.` });
      void queryClient.invalidateQueries({ queryKey: queryKeys.users });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to remove user";
      setMessage({ type: "error", text });
    }
  };

  const toggleCapability = async (user: DemoUser, capability: string) => {
    if (!canManageUsers) {
      setMessage({ type: "error", text: "Insufficient permissions: users.manage is required to manage users." });
      return;
    }
    const currentCapabilities = user.capabilities ?? [];
    const capabilities = currentCapabilities.includes(capability)
      ? currentCapabilities.filter((item) => item !== capability)
      : [...currentCapabilities, capability];
    try {
      await putJson(`/api/users/${user.id}`, { capabilities });
      setMessage({ type: "success", text: "Updated capabilities." });
      void queryClient.invalidateQueries({ queryKey: queryKeys.users });
      if (currentUser?.id === user.id) {
        void refreshAuth();
      }
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to update user";
      setMessage({ type: "error", text });
    }
  };

  const addCapability = async (user: DemoUser) => {
    if (!canManageUsers) {
      setMessage({ type: "error", text: "Insufficient permissions: users.manage is required to manage users." });
      return;
    }
    const draft = (capDrafts[user.id] ?? "").trim();
    if (!draft) return;
    const currentCapabilities = user.capabilities ?? [];
    if (currentCapabilities.includes(draft)) {
      setCapDrafts((prev) => ({ ...prev, [user.id]: "" }));
      return;
    }
    try {
      await putJson(`/api/users/${user.id}`, { capabilities: [...currentCapabilities, draft] });
      setCapDrafts((prev) => ({ ...prev, [user.id]: "" }));
      setMessage({ type: "success", text: `Added capability ${draft}.` });
      void queryClient.invalidateQueries({ queryKey: queryKeys.users });
      if (currentUser?.id === user.id) {
        void refreshAuth();
      }
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to update user";
      setMessage({ type: "error", text });
    }
  };

  return (
    <div className="space-y-5">
      <PageHeaderCard
        title="Users & Permissions"
        description="Manage who can view, operate, and administer this controller."
        actions={
          <NodeButton
            variant="primary"
            onClick={() => (canManageUsers ? setShowModal(true) : null)}
            disabled={!canManageUsers}
            title={canManageUsers ? undefined : "Requires users.manage"}
          >
            Add user
          </NodeButton>
        }
      >
        {!canManageUsers ? (
 <p className="mt-3 text-xs text-muted-foreground">
            Read-only: you need <code className="px-1">users.manage</code> to add/remove users or edit capabilities.
          </p>
        ) : null}
      </PageHeaderCard>

      {message && (
        <InlineBanner tone={message.type === "success" ? "success" : "error"}>
          {message.text}
        </InlineBanner>
      )}

      <CollapsibleCard
        title="Users"
        description="Manage accounts and capabilities."
        defaultOpen
        className="overflow-hidden"
        bodyClassName="px-0 py-0"
      >
        <div className="overflow-x-auto">
          <table className="min-w-full divide-y divide-border text-sm">
            <thead className="bg-card-inset">
              <tr>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Name
                </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Email
                </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Role
                </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Capabilities
                </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Last login
                </th>
                <th className="px-4 py-3" aria-label="actions" />
              </tr>
            </thead>
            <tbody className="divide-y divide-border">
              {users.map((user) => {
                const userCapabilities = user.capabilities ?? [];
                return (
 <tr key={user.id} className="hover:bg-muted">
 <td className="px-4 py-3 font-medium text-foreground">
                      {user.name}
                    </td>
 <td className="px-4 py-3 text-muted-foreground">
                      {user.email}
                    </td>
 <td className="px-4 py-3 text-muted-foreground uppercase">
                      {user.role}
                    </td>
 <td className="px-4 py-3 text-muted-foreground">
                      <div className="space-y-2">
                        <div className="flex flex-wrap gap-2">
                          {KNOWN_CAPABILITIES.map((capability) => {
                            const enabled = userCapabilities.includes(capability);
                            return (
                              <button
                                key={capability}
                                onClick={() => toggleCapability(user, capability)}
                                disabled={!canManageUsers}
                                className={`inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-semibold transition-colors ${
                                  enabled
                                    ? "bg-indigo-600 text-white hover:bg-indigo-700"
 : "border border-border bg-white text-muted-foreground hover:bg-muted"
                                } disabled:pointer-events-none disabled:opacity-50`}
                              >
                                {capability}
                              </button>
                            );
                          })}
                          {userCapabilities
                            .filter((capability) => !KNOWN_CAPABILITIES.includes(capability))
                            .map((capability) => (
                              <button
                                key={capability}
                                onClick={() => toggleCapability(user, capability)}
                                disabled={!canManageUsers}
 className="inline-flex items-center rounded-full border border-border bg-white px-2.5 py-0.5 text-xs font-semibold text-muted-foreground hover:bg-muted disabled:pointer-events-none disabled:opacity-50"
                              >
                                {capability} ×
                              </button>
                            ))}
                        </div>
                        <div className="flex gap-2">
                          <Input
                            value={capDrafts[user.id] ?? ""}
                            onChange={(event) =>
                              setCapDrafts((prev) => ({ ...prev, [user.id]: event.target.value }))
                            }
                            disabled={!canManageUsers}
                            placeholder="Add capability…"
                          />
                          <NodeButton
                            type="button"
                            size="xs"
                            onClick={() => void addCapability(user)}
                            disabled={!canManageUsers}
                          >
                            Add
                          </NodeButton>
                        </div>
                      </div>
                    </td>
 <td className="px-4 py-3 text-muted-foreground">
                      {user.last_login
                        ? formatDistanceToNow(new Date(user.last_login), { addSuffix: true })
                        : "–"}
                    </td>
                    <td className="px-4 py-3 text-right">
                      <div className="flex flex-wrap justify-end gap-2">
                        <NodeButton
                          type="button"
                          size="xs"
                          onClick={() => (canManageUsers ? setPasswordTarget(user) : null)}
                          disabled={!canManageUsers}
                          title={canManageUsers ? undefined : "Requires users.manage"}
                        >
                          Set password
                        </NodeButton>
                        <button
                          type="button"
                          onClick={() => removeUser(user)}
                          disabled={!canManageUsers}
 className="inline-flex items-center justify-center rounded-lg border border-rose-200 bg-white px-3 py-2 text-xs font-semibold text-rose-700 shadow-xs hover:bg-rose-50 focus:outline-hidden focus:bg-rose-50 disabled:pointer-events-none disabled:opacity-50"
                        >
                          Remove
                        </button>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </CollapsibleCard>

      <AddUserModal
        open={showModal}
        onClose={() => setShowModal(false)}
        onCreated={(text) => {
          setMessage({ type: "success", text });
          setShowModal(false);
          void queryClient.invalidateQueries({ queryKey: queryKeys.users });
        }}
        onError={(text) => setMessage({ type: "error", text })}
      />

      <SetPasswordModal
        user={passwordTarget}
        onClose={() => setPasswordTarget(null)}
        onSuccess={(text, isCurrentUser) => {
          setMessage({ type: "success", text });
          setPasswordTarget(null);
          void queryClient.invalidateQueries({ queryKey: queryKeys.users });
          if (isCurrentUser) {
            void refreshAuth();
          }
        }}
        onError={(text) => setMessage({ type: "error", text })}
      />
    </div>
  );
}

function AddUserModal({
  open,
  onClose,
  onCreated,
  onError,
}: {
  open: boolean;
  onClose: () => void;
  onCreated: (text: string) => void;
  onError: (text: string) => void;
}) {
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [role, setRole] = useState<"admin" | "operator" | "view">("view");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fieldId = useId();
  const nameId = `${fieldId}-name`;
  const emailId = `${fieldId}-email`;
  const passwordId = `${fieldId}-password`;
  const roleId = `${fieldId}-role`;

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!name.trim() || !email.trim() || !password.trim()) {
      setError("Name, email, and password are required");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const body = {
        name: name.trim(),
        email: email.trim(),
        password: password.trim(),
        role,
        capabilities: ROLE_CAPABILITIES[role],
      };
      const created = await postJson<DemoUser>("/api/users", body);
      onCreated(`Created ${created.name}.`);
      setName("");
      setEmail("");
      setPassword("");
      setRole("view");
      onClose();
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to create user";
      setError(text);
      onError(text);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) onClose(); }}>
      <DialogContent className="gap-0">
        <DialogTitle>Add user</DialogTitle>
        <form className="mt-4 space-y-4" onSubmit={handleSubmit}>
          <div>
            <label
              htmlFor={nameId}
 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground"
            >
              Name
            </label>
            <Input
              id={nameId}
              value={name}
              onChange={(event) => setName(event.target.value)}
              className="mt-1"
              autoComplete="name"
            />
          </div>
          <div>
            <label
              htmlFor={emailId}
 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground"
            >
              Email
            </label>
            <Input
              id={emailId}
              value={email}
              onChange={(event) => setEmail(event.target.value)}
              className="mt-1"
              type="email"
              autoComplete="email"
            />
          </div>
          <div>
            <label
              htmlFor={passwordId}
 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground"
            >
              Password
            </label>
            <Input
              id={passwordId}
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              className="mt-1"
              type="password"
              autoComplete="new-password"
            />
          </div>
          <div>
            <label
              htmlFor={roleId}
 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground"
            >
              Role
            </label>
            <Select
              id={roleId}
              value={role}
              onChange={(event) => setRole(event.target.value as typeof role)}
              className="mt-1"
            >
              <option value="admin">Admin</option>
              <option value="operator">Operator</option>
              <option value="view">View</option>
            </Select>
          </div>
          {error && (
            <InlineBanner tone="danger" className="px-3 py-2 text-xs">
              {error}
            </InlineBanner>
          )}
          <div className="flex items-center justify-end gap-3">
            <NodeButton type="button" onClick={onClose}>
              Cancel
            </NodeButton>
            <NodeButton
              type="submit"
              disabled={busy}
              variant="primary"
            >
              {busy ? "Creating…" : "Create"}
            </NodeButton>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function SetPasswordModal({
  user,
  onClose,
  onSuccess,
  onError,
}: {
  user: DemoUser | null;
  onClose: () => void;
  onSuccess: (text: string, isCurrentUser: boolean) => void;
  onError: (text: string) => void;
}) {
  const { me: currentUser } = useAuth();
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fieldId = useId();
  const passwordId = `${fieldId}-new-password`;

  if (!user) return null;

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!password.trim()) {
      setError("Password is required");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await putJson(`/api/users/${user.id}`, { password: password.trim() });
      setPassword("");
      onSuccess(`Updated password for ${user.email}.`, currentUser?.id === user.id);
      onClose();
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to update password";
      setError(text);
      onError(text);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={!!user} onOpenChange={(v) => { if (!v) onClose(); }}>
      <DialogContent className="gap-0">
        <DialogTitle>Set password</DialogTitle>
        <DialogDescription className="mt-1">
          Update the password for{" "}
 <span className="font-medium text-foreground">{user.email}</span>.
        </DialogDescription>
        <form className="mt-4 space-y-4" onSubmit={handleSubmit}>
          <div>
            <label
              htmlFor={passwordId}
 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground"
            >
              New password
            </label>
            <Input
              id={passwordId}
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              className="mt-1"
              type="password"
              autoComplete="new-password"
            />
          </div>
          {error ? (
            <InlineBanner tone="danger" className="px-3 py-2 text-xs">
              {error}
            </InlineBanner>
          ) : null}
          <div className="flex justify-end gap-2">
            <NodeButton type="button" onClick={onClose}>
              Cancel
            </NodeButton>
            <NodeButton
              type="submit"
              disabled={busy}
              variant="primary"
            >
              {busy ? "Saving…" : "Save password"}
            </NodeButton>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
