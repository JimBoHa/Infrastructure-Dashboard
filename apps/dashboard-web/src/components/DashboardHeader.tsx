"use client";

import Link from "next/link";
import { useAuth } from "@/components/AuthProvider";
import { useDashboardUi } from "@/components/DashboardUiProvider";
import { useConnectionQuery } from "@/lib/queries";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

export default function DashboardHeader() {
  const { me, logout } = useAuth();
  const { sidebarOpen, toggleSidebar } = useDashboardUi();
  const { data: connection } = useConnectionQuery();

  return (
    <header className="fixed top-0 inset-x-0 z-50 flex h-14 items-center border-b border-border bg-card px-4">
      <div className="flex w-full items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <button
            type="button"
            className="inline-flex size-9 items-center justify-center rounded-lg border border-border text-card-foreground hover:bg-accent focus:outline-hidden focus:bg-accent [@media(min-width:1024px)_and_(pointer:fine)]:hidden"
            aria-haspopup="dialog"
            aria-expanded={sidebarOpen}
            aria-controls="dashboard-sidebar"
            onClick={toggleSidebar}
            aria-label={sidebarOpen ? "Close navigation" : "Open navigation"}
          >
            <svg
              className="size-4"
              xmlns="http://www.w3.org/2000/svg"
              width="24"
              height="24"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <line x1="3" y1="6" x2="21" y2="6" />
              <line x1="3" y1="12" x2="21" y2="12" />
              <line x1="3" y1="18" x2="21" y2="18" />
            </svg>
          </button>

          <Link href="/nodes" className="flex items-center gap-2">
            <span className="inline-flex size-9 items-center justify-center rounded-xl bg-indigo-600 text-sm font-semibold text-white">
              FD
            </span>
            <span className="hidden text-sm font-semibold text-card-foreground sm:inline">
              Farm Dashboard
            </span>
          </Link>
        </div>

        <div className="flex items-center gap-2">
          {connection ? (
            <span className="hidden items-center gap-2 rounded-full bg-accent px-3 py-1 text-xs font-medium text-card-foreground sm:inline-flex">
              <span
                className={
                  connection.status === "online"
                    ? "size-2 rounded-full bg-emerald-500"
                    : "size-2 rounded-full bg-gray-400"
                }
                aria-hidden
              />
              {connection.mode} · {connection.status}
            </span>
          ) : null}

          {me ? (
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <button
                  type="button"
                  className="inline-flex items-center gap-x-2 rounded-full border border-border bg-background px-3 py-1.5 text-xs font-medium text-card-foreground shadow-xs hover:bg-accent focus:outline-hidden focus:bg-accent"
                  aria-label="Account menu"
                >
                  <span className="hidden max-w-[220px] truncate sm:inline">{me.email}</span>
                  <span className="inline-flex size-7 items-center justify-center rounded-full bg-indigo-600 text-xs font-semibold text-white sm:hidden">
                    {me.email.slice(0, 1).toUpperCase()}
                  </span>
                  <svg
                    className="size-4 text-muted-foreground"
                    xmlns="http://www.w3.org/2000/svg"
                    width="24"
                    height="24"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <path d="m6 9 6 6 6-6" />
                  </svg>
                </button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-64">
                <DropdownMenuLabel className="font-normal">
                  <p className="truncate text-sm font-semibold text-card-foreground">
                    {me.email}
                  </p>
                  <p className="text-xs text-muted-foreground">{me.role}</p>
                </DropdownMenuLabel>
                <DropdownMenuSeparator />
                <DropdownMenuItem onSelect={logout}>
                  Log out
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          ) : null}
        </div>
      </div>
    </header>
  );
}
