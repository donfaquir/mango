import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { ReactFlowProvider } from "@xyflow/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { TooltipProvider } from "@/components/ui/tooltip";

const undo = vi.fn();
const redo = vi.fn();
const createCheckpoint = vi.fn();
const toastSuccess = vi.fn();
const toastError = vi.fn();

let pastLen = 0;
let futureLen = 0;
let isPending = false;

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
  toast: {
    success: (msg: string) => toastSuccess(msg),
    error: (msg: string) => toastError(msg),
  },
}));

vi.mock("@/hooks/useCheckpoint", () => ({
  useCreateCheckpoint: () => ({
    mutateAsync: createCheckpoint,
    isPending,
  }),
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
    createCheckpoint.mockReset();
    toastSuccess.mockReset();
    toastError.mockReset();
    pastLen = 0;
    futureLen = 0;
    isPending = false;
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

  it("save-version flushes autoSave then mutates and toasts success", async () => {
    const order: string[] = [];
    const onBeforeSaveVersion = vi.fn(() => {
      order.push("flush");
    });
    createCheckpoint.mockImplementation(async () => {
      order.push("mutate");
      return { id: "cp1" };
    });

    renderToolbar({ onBeforeSaveVersion });
    fireEvent.click(screen.getByRole("button", { name: /保存版本/ }));

    await waitFor(() => expect(toastSuccess).toHaveBeenCalledWith("已保存版本"));
    expect(onBeforeSaveVersion).toHaveBeenCalledTimes(1);
    expect(createCheckpoint).toHaveBeenCalledWith({
      episode_id: "ep1",
      label: null,
    });
    expect(order).toEqual(["flush", "mutate"]);
  });

  it("save-version surfaces a toast error when the mutation rejects", async () => {
    createCheckpoint.mockRejectedValueOnce(new Error("boom"));
    renderToolbar();
    fireEvent.click(screen.getByRole("button", { name: /保存版本/ }));
    await waitFor(() => expect(toastError).toHaveBeenCalled());
    expect(toastSuccess).not.toHaveBeenCalled();
  });

  it("disables the save-version button while the mutation is pending", () => {
    isPending = true;
    renderToolbar();
    expect(screen.getByRole("button", { name: /保存版本/ })).toBeDisabled();
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
