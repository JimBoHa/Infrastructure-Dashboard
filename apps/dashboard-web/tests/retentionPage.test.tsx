import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import RetentionPage from "@/app/(dashboard)/backups/page";
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
const mockUpdateBackupRetentionPolicies = vi.fn();

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

vi.mock("@/lib/api", async (orig) => {
  const actual = await orig();
  return {
    ...actual,
    updateBackupRetentionPolicies: (...args: unknown[]) =>
      mockUpdateBackupRetentionPolicies(...args),
  };
});

const demoNodes: DemoNode[] = [
  { id: "node-1", name: "Barn Controller" },
  { id: "node-2", name: "Field Node" },
];

const demoBackups: DemoBackup[] = [
  {
    id: "backup-1",
    node_id: "node-1",
    captured_at: "2024-05-01T12:00:00Z",
    size_bytes: 2048,
    path: "/backups/node-1/2024-05-01.tar.gz",
  },
];

describe("RetentionPage", () => {
  beforeEach(() => {
    mockUseBackupRetentionConfigQuery.mockReturnValue({
      data: {
        default_keep_days: 30,
        policies: [
          { node_id: "node-1", keep_days: 45, node_name: "Barn Controller" },
        ],
      },
      error: null,
      isLoading: false,
    });
    mockUpdateBackupRetentionPolicies.mockResolvedValue({
      default_keep_days: 30,
      policies: [
        { node_id: "node-1", keep_days: 60, node_name: "Barn Controller" },
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
    mockUseRecentRestoresQuery.mockReturnValue({ data: [] });
  });

  it("renders retention table with defaults", () => {
    render(<RetentionPage />);
    expect(screen.getByText(/Retention policies/i)).toBeInTheDocument();
    expect(screen.getByDisplayValue("45")).toBeInTheDocument();
  });

  it("submits retention update", async () => {
    render(<RetentionPage />);
    const input = screen.getByLabelText(/Retention days for Barn Controller/i);
    fireEvent.change(input, { target: { value: "60" } });
    const saveButtons = screen.getAllByText("Save");
    fireEvent.click(saveButtons[0]);
    await waitFor(() => expect(mockUpdateBackupRetentionPolicies).toHaveBeenCalled());
    const [payload] = mockUpdateBackupRetentionPolicies.mock.calls[0];
    expect(payload).toEqual([{ node_id: "node-1", keep_days: 60 }]);
  });
});
