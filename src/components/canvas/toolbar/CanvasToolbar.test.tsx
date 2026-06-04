import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { TooltipProvider } from "@/components/ui/tooltip";

const undo = vi.fn();
const redo = vi.fn();

let pastLen = 0;
let futureLen = 0;

vi.mock("@/stores/canvasStore", () => ({
  useCanvasStore: {
    temporal: {
      getState: () => ({
        pastStates: new Array(pastLen).fill(null),
        futureStates: new Array(futureLen).fill(null),
        undo,
        redo,
      }),
      subscribe: () => () => {},
    },
  },
}));

vi.mock("sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

vi.mock("@/hooks/useCheckpoint", () => ({
  useCreateCheckpoint: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useCheckpointList: () => ({ data: [], isLoading: false }),
  useDeleteCheckpoint: () => ({ mutateAsync: vi.fn(), isPending: false }),
  useRestoreCheckpoint: () => ({ mutateAsync: vi.fn(), isPending: false }),
}));

import { CanvasToolbar } from "./CanvasToolbar";

function renderToolbar(props: { onBeforeSaveVersion?: () => Promise<void> | void } = {}) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <TooltipProvider>
        <ReactFlowProvider>
          <CanvasToolbar episodeId="ep1" {...props} />
        </ReactFlowProvider>
      </TooltipProvider>
    </QueryClientProvider>,
  );
}

describe("CanvasToolbar", () => {
  beforeEach(() => {
    undo.mockReset();
    redo.mockReset();
    pastLen = 0;
    futureLen = 0;
  });

  it("disables undo when there is no past, enables when there is", () => {
    pastLen = 0;
    const { unmount } = renderToolbar();
    expect(screen.getByRole("button", { name: /撤销/ })).toBeDisabled();
    unmount();
    pastLen = 2;
    renderToolbar();
    expect(screen.getByRole("button", { name: /撤销/ })).toBeEnabled();
  });

  it("disables redo when there is no future, enables when there is", () => {
    futureLen = 0;
    const { unmount } = renderToolbar();
    expect(screen.getByRole("button", { name: /重做/ })).toBeDisabled();
    unmount();
    futureLen = 1;
    renderToolbar();
    expect(screen.getByRole("button", { name: /重做/ })).toBeEnabled();
  });

  it("clicking undo calls the temporal undo", () => {
    pastLen = 1;
    renderToolbar();
    fireEvent.click(screen.getByRole("button", { name: /撤销/ }));
    expect(undo).toHaveBeenCalledTimes(1);
  });

  it("clicking redo calls the temporal redo", () => {
    futureLen = 1;
    renderToolbar();
    fireEvent.click(screen.getByRole("button", { name: /重做/ }));
    expect(redo).toHaveBeenCalledTimes(1);
  });

  it("save-version button opens the SaveVersionDialog", () => {
    renderToolbar();
    fireEvent.click(screen.getByRole("button", { name: /保存版本/ }));
    expect(screen.getByRole("heading", { name: "保存版本" })).toBeInTheDocument();
  });

  it("version-history button opens the VersionHistoryDialog", () => {
    renderToolbar();
    fireEvent.click(screen.getByRole("button", { name: /版本历史/ }));
    expect(screen.getByRole("heading", { name: "版本历史" })).toBeInTheDocument();
  });

  it("toggles canvas interactivity between lock and unlock", () => {
    renderToolbar();
    const lockBtn = screen.getByRole("button", { name: "锁定画布" });
    fireEvent.click(lockBtn);
    expect(screen.getByRole("button", { name: "解锁画布" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "解锁画布" }));
    expect(screen.getByRole("button", { name: "锁定画布" })).toBeInTheDocument();
  });
});
