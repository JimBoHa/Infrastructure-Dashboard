import { render, screen } from "@testing-library/react";
import { vi } from "vitest";
import BackupsPage from "@/app/(dashboard)/backups/page";
import type { DemoBackup, DemoNode } from "@/types/dashboard";

vi.mock("@/components/AuthProvider", () => ({
  useAuth: () => ({
    ready: true,
    token: "test-token",
    me: { capabilities: ["config.write"] },
    refresh: vi.fn(),
    login: vi.fn(),
    logout: vi.fn(),
  }),
}));

const mockUseNodesQuery = vi.fn();
const mockUseBackupsQuery = vi.fn();
const mockUseRecentRestoresQuery = vi.fn();
const mockUseBackupRetentionConfigQuery = vi.fn();

vi.mock("@/lib/queries", () => ({
  queryKeys: {
    backups: ["backups"],
    recentRestores: ["backups", "recent-restores"],
    backupRetention: ["backups", "retention"],
  },
  useNodesQuery: () => mockUseNodesQuery(),
  useBackupsQuery: () => mockUseBackupsQuery(),
  useRecentRestoresQuery: () => mockUseRecentRestoresQuery(),
  useBackupRetentionConfigQuery: () => mockUseBackupRetentionConfigQuery(),
}));

vi.mock("@tanstack/react-query", () => ({
  useQueryClient: () => ({
    invalidateQueries: vi.fn(),
    setQueryData: vi.fn(),
  }),
}));

const demoNodes: DemoNode[] = [
  { id: "node-1", name: "Barn Controller" },
  { id: "node-2", name: "Field Node" },
];

const demoBackups: DemoBackup[] = [
  {
    id: "b1",
    node_id: "node-1",
    captured_at: "2025-03-01T00:00:00Z",
    size_bytes: 1024,
    path: "/backups/node-1/2025-03-01.json",
  },
];

describe("BackupsPage restore activity", () => {
  beforeEach(() => {
    mockUseBackupRetentionConfigQuery.mockReturnValue({
      data: {
        default_keep_days: 30,
        policies: [],
        last_cleanup_at: null,
      },
      error: null,
      isLoading: false,
    });
    mockUseRecentRestoresQuery.mockReturnValue({
      data: [
        {
          backup_node_id: "node-1",
          date: "2025-03-01",
          recorded_at: "2025-03-01T12:00:00Z",
          status: "queued",
        },
      ],
    });
    mockUseNodesQuery.mockReturnValue({
      data: demoNodes,
      error: null,
      isLoading: false,
    });
    mockUseBackupsQuery.mockReturnValue({
      data: demoBackups,
      error: null,
      isLoading: false,
    });
  });

  it("renders restore activity when recent restores are available", () => {
    render(<BackupsPage />);
    expect(screen.getByText(/Restore activity/i)).toBeInTheDocument();
    expect(screen.getByText(/Restore queued for/)).toBeInTheDocument();
  });
});
