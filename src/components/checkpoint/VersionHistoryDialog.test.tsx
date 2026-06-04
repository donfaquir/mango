import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { EpisodeCheckpointListItem } from "@/lib/bindings/commands";

const mockItems: EpisodeCheckpointListItem[] = [
  {
    id: "cp1",
    episode_id: "ep1",
    version_number: 2,
    trigger_type: "manual",
    label: "分镜定稿",
    change_summary: "新增 2 个分镜（共 10）",
    created_at: "2026-06-04 10:00:00",
  },
  {
    id: "cp2",
    episode_id: "ep1",
    version_number: 1,
    trigger_type: "auto",
    label: null,
    change_summary: null,
    created_at: "2026-06-04 09:00:00",
  },
];

vi.mock("@/hooks/useCheckpoint", () => ({
  useCheckpointList: () => ({ data: mockItems, isLoading: false }),
  useDeleteCheckpoint: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useRestoreCheckpoint: () => ({ mutateAsync: vi.fn(), isPending: false }),
}));

vi.mock("sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

import { VersionHistoryDialog } from "./VersionHistoryDialog";

function renderDialog() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <VersionHistoryDialog
        open={true}
        onOpenChange={vi.fn()}
        episodeId="ep1"
      />
    </QueryClientProvider>,
  );
}

describe("VersionHistoryDialog", () => {
  it("renders manual checkpoint with version number and label", () => {
    renderDialog();
    expect(screen.getByText("版本 2 · 分镜定稿")).toBeInTheDocument();
  });

  it("renders auto checkpoint as '自动备份'", () => {
    renderDialog();
    expect(screen.getByText("自动备份")).toBeInTheDocument();
  });

  it("shows change_summary for manual checkpoint", () => {
    renderDialog();
    expect(screen.getByText("新增 2 个分镜（共 10）")).toBeInTheDocument();
  });

  it("shows '定时保存' when auto checkpoint has no summary", () => {
    renderDialog();
    expect(screen.getByText("定时保存")).toBeInTheDocument();
  });

  it("shows empty state when no items", () => {
    vi.doMock("@/hooks/useCheckpoint", () => ({
      useCheckpointList: () => ({ data: [], isLoading: false }),
      useDeleteCheckpoint: () => ({ mutateAsync: vi.fn(), isPending: false }),
      useRestoreCheckpoint: () => ({ mutateAsync: vi.fn(), isPending: false }),
    }));
  });
});
