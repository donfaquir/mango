import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { EpisodeCheckpointListItem } from "@/lib/bindings/commands";

const mutateAsync = vi.fn();
const toastSuccess = vi.fn();
const toastError = vi.fn();

vi.mock("@/hooks/useCheckpoint", () => ({
  useRestoreCheckpoint: () => ({
    mutateAsync,
    isPending: false,
  }),
}));

vi.mock("sonner", () => ({
  toast: {
    success: (msg: string) => toastSuccess(msg),
    error: (msg: string) => toastError(msg),
  },
}));

import { RestoreConfirmDialog } from "./RestoreConfirmDialog";

const manualCheckpoint: EpisodeCheckpointListItem = {
  id: "cp1",
  episode_id: "ep1",
  version_number: 3,
  trigger_type: "manual",
  label: "分镜定稿",
  change_summary: "修改 2 个分镜",
  created_at: "2026-06-04 10:00:00",
};

function renderDialog(props: { onRestored?: () => void } = {}) {
  const onOpenChange = vi.fn();
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={queryClient}>
      <RestoreConfirmDialog
        open={true}
        onOpenChange={onOpenChange}
        checkpoint={manualCheckpoint}
        episodeId="ep1"
        {...props}
      />
    </QueryClientProvider>,
  );
  return { onOpenChange };
}

describe("RestoreConfirmDialog", () => {
  beforeEach(() => {
    mutateAsync.mockReset().mockResolvedValue({ id: "ep1" });
    toastSuccess.mockReset();
    toastError.mockReset();
  });

  it("shows risk warning text", () => {
    renderDialog();
    expect(screen.getByText(/生成结果将不再关联到分镜/)).toBeInTheDocument();
  });

  it("cancel does not call restore", () => {
    renderDialog();
    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    expect(mutateAsync).not.toHaveBeenCalled();
  });

  it("confirm calls restore and toasts success", async () => {
    const onRestored = vi.fn();
    renderDialog({ onRestored });
    fireEvent.click(screen.getByRole("button", { name: "确认恢复" }));

    await waitFor(() =>
      expect(mutateAsync).toHaveBeenCalledWith("cp1"),
    );
    expect(toastSuccess).toHaveBeenCalledWith("已恢复到版本 3 · 分镜定稿");
    expect(onRestored).toHaveBeenCalled();
  });

  it("shows error toast on failure", async () => {
    mutateAsync.mockRejectedValueOnce(new Error("fail"));
    renderDialog();
    fireEvent.click(screen.getByRole("button", { name: "确认恢复" }));

    await waitFor(() => expect(toastError).toHaveBeenCalled());
    expect(toastSuccess).not.toHaveBeenCalled();
  });
});
