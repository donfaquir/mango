import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

const screenToFlowPosition = vi.fn();
vi.mock("@xyflow/react", () => ({
  useReactFlow: () => ({ screenToFlowPosition }),
}));

const mutateAsync = vi.fn();
vi.mock("@/hooks/useAssets", () => ({
  useImportAsset: () => ({ mutateAsync }),
}));

type Position = { x: number; y: number };
type Handler = (paths: string[], position: Position) => void | Promise<void>;
const useGlobalDropTarget = vi.fn();
vi.mock("@/hooks/useGlobalDropTarget", () => ({
  useGlobalDropTarget: (...args: unknown[]) => useGlobalDropTarget(...args),
}));

const toastError = vi.fn();
const toastWarning = vi.fn();
const toastSuccess = vi.fn();
vi.mock("sonner", () => ({
  toast: {
    error: (...args: unknown[]) => toastError(...args),
    warning: (...args: unknown[]) => toastWarning(...args),
    success: (...args: unknown[]) => toastSuccess(...args),
  },
}));

import { useSystemFileDrop } from "./useSystemFileDrop";

function captureHandler(): Handler {
  const call = useGlobalDropTarget.mock.calls[useGlobalDropTarget.mock.calls.length - 1];
  return call?.[0] as Handler;
}

beforeEach(() => {
  screenToFlowPosition.mockReset();
  mutateAsync.mockReset();
  useGlobalDropTarget.mockReset();
  toastError.mockReset();
  toastWarning.mockReset();
  toastSuccess.mockReset();
  Object.defineProperty(window, "devicePixelRatio", {
    value: 2,
    configurable: true,
  });
  screenToFlowPosition.mockImplementation(({ x, y }: Position) => ({ x, y }));
});

describe("useSystemFileDrop", () => {
  it("registers a stable handler with useGlobalDropTarget", () => {
    const onPlaceAsset = vi.fn();
    const { rerender } = renderHook(() =>
      useSystemFileDrop({ projectId: "p1", onPlaceAsset }),
    );
    const first = captureHandler();
    rerender();
    const second = captureHandler();
    expect(first).toBe(second);
  });

  it("divides physical pixels by devicePixelRatio before screenToFlowPosition", async () => {
    mutateAsync.mockResolvedValueOnce({ id: "a1" });
    const onPlaceAsset = vi.fn();
    renderHook(() => useSystemFileDrop({ projectId: "p1", onPlaceAsset }));

    await captureHandler()(["/tmp/a.png"], { x: 200, y: 100 });

    expect(screenToFlowPosition).toHaveBeenCalledWith({ x: 100, y: 50 });
    expect(mutateAsync).toHaveBeenCalledWith({
      project_id: "p1",
      source_path: "/tmp/a.png",
      shot_id: null,
    });
    expect(onPlaceAsset).toHaveBeenCalledWith("a1", { x: 100, y: 50 });
  });

  it("warns and skips unsupported files in a mixed batch", async () => {
    mutateAsync.mockResolvedValueOnce({ id: "a-png" });
    const onPlaceAsset = vi.fn();
    renderHook(() => useSystemFileDrop({ projectId: "p1", onPlaceAsset }));

    await captureHandler()(["/x/a.png", "/x/b.txt"], { x: 0, y: 0 });

    expect(toastWarning).toHaveBeenCalledWith("已忽略 1 个不支持的文件");
    expect(mutateAsync).toHaveBeenCalledTimes(1);
    expect(mutateAsync).toHaveBeenCalledWith({
      project_id: "p1",
      source_path: "/x/a.png",
      shot_id: null,
    });
    expect(onPlaceAsset).toHaveBeenCalledWith("a-png", { x: 0, y: 0 });
  });

  it("errors and short-circuits when no supported files were dropped", async () => {
    const onPlaceAsset = vi.fn();
    renderHook(() => useSystemFileDrop({ projectId: "p1", onPlaceAsset }));

    await captureHandler()(["/x/note.txt"], { x: 0, y: 0 });

    expect(toastError).toHaveBeenCalledTimes(1);
    expect(mutateAsync).not.toHaveBeenCalled();
    expect(onPlaceAsset).not.toHaveBeenCalled();
  });

  it("staggers successive nodes by 20px on each axis", async () => {
    mutateAsync
      .mockResolvedValueOnce({ id: "a-1" })
      .mockResolvedValueOnce({ id: "a-2" })
      .mockResolvedValueOnce({ id: "a-3" });
    const onPlaceAsset = vi.fn();
    renderHook(() => useSystemFileDrop({ projectId: "p1", onPlaceAsset }));

    await captureHandler()(
      ["/x/a.png", "/x/b.jpg", "/x/c.webp"],
      { x: 0, y: 0 },
    );

    expect(onPlaceAsset).toHaveBeenNthCalledWith(1, "a-1", { x: 0, y: 0 });
    expect(onPlaceAsset).toHaveBeenNthCalledWith(2, "a-2", { x: 20, y: 20 });
    expect(onPlaceAsset).toHaveBeenNthCalledWith(3, "a-3", { x: 40, y: 40 });
  });

  it("toasts per-file failures and keeps processing the rest", async () => {
    mutateAsync
      .mockRejectedValueOnce(new Error("disk full"))
      .mockResolvedValueOnce({ id: "ok" });
    const onPlaceAsset = vi.fn();
    renderHook(() => useSystemFileDrop({ projectId: "p1", onPlaceAsset }));

    await captureHandler()(["/x/a.png", "/x/b.png"], { x: 0, y: 0 });

    await waitFor(() => expect(toastError).toHaveBeenCalled());
    expect(toastError.mock.calls[0]?.[0]).toContain("a.png");
    expect(toastError.mock.calls[0]?.[0]).toContain("disk full");
    expect(onPlaceAsset).toHaveBeenCalledTimes(1);
    expect(onPlaceAsset).toHaveBeenCalledWith("ok", { x: 20, y: 20 });
  });

  it("propagates the enabled flag to useGlobalDropTarget", () => {
    renderHook(() =>
      useSystemFileDrop({
        projectId: "p1",
        onPlaceAsset: vi.fn(),
        enabled: false,
      }),
    );
    expect(useGlobalDropTarget).toHaveBeenLastCalledWith(
      expect.any(Function),
      false,
    );
  });
});
