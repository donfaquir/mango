import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook } from "@testing-library/react";
import { createElement, type ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const upsertCanvasLayout = vi.fn();

vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    upsertCanvasLayout: (...args: unknown[]) => upsertCanvasLayout(...args),
  },
}));

import { useCanvasStore } from "@/stores/canvasStore";
import { useAutoSaveLayout } from "./useAutoSave";

function makeWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const wrapper = ({ children }: { children: ReactNode }) =>
    createElement(QueryClientProvider, { client: queryClient }, children);
  return { wrapper };
}

describe("useAutoSaveLayout", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    upsertCanvasLayout.mockReset();
    upsertCanvasLayout.mockResolvedValue({
      status: "ok",
      data: {
        id: "cl1",
        episode_id: "e1",
        nodes_json: "[]",
        edges_json: "[]",
        viewport_json: "{\"x\":0,\"y\":0,\"zoom\":1}",
        updated_at: "2026-05-27 12:00:00",
      },
    });
    useCanvasStore.getState().reset();
  });

  afterEach(() => {
    vi.useRealTimers();
    useCanvasStore.getState().reset();
  });

  it("debounces writes and persists ~1s after a store change", async () => {
    const { wrapper } = makeWrapper();
    renderHook(() => useAutoSaveLayout("e1"), { wrapper });

    act(() => {
      useCanvasStore.getState().init({
        nodes: [{ id: "n1", position: { x: 0, y: 0 }, data: {} }],
        edges: [],
        viewport: { x: 0, y: 0, zoom: 1 },
      });
    });

    expect(upsertCanvasLayout).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1100);
    });

    expect(upsertCanvasLayout).toHaveBeenCalledTimes(1);
    expect(upsertCanvasLayout).toHaveBeenCalledWith(
      expect.objectContaining({ episode_id: "e1" }),
    );
  });

  it("skips writing when the layout reverts to its previous hash", async () => {
    const { wrapper } = makeWrapper();
    renderHook(() => useAutoSaveLayout("e1"), { wrapper });

    const nodeA = { id: "n1", position: { x: 0, y: 0 }, data: {} };

    act(() => {
      useCanvasStore.getState().init({
        nodes: [nodeA],
        edges: [],
        viewport: { x: 0, y: 0, zoom: 1 },
      });
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1100);
    });
    expect(upsertCanvasLayout).toHaveBeenCalledTimes(1);

    // Add, then remove → same content as the last saved state.
    act(() => {
      useCanvasStore.getState().init({
        nodes: [
          nodeA,
          { id: "n2", position: { x: 5, y: 5 }, data: {} },
        ],
        edges: [],
        viewport: { x: 0, y: 0, zoom: 1 },
      });
    });
    act(() => {
      useCanvasStore.getState().init({
        nodes: [nodeA],
        edges: [],
        viewport: { x: 0, y: 0, zoom: 1 },
      });
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1100);
    });

    expect(upsertCanvasLayout).toHaveBeenCalledTimes(1);
  });

  it("collapses many rapid changes into one write", async () => {
    const { wrapper } = makeWrapper();
    renderHook(() => useAutoSaveLayout("e1"), { wrapper });

    for (let i = 0; i < 5; i++) {
      act(() => {
        useCanvasStore.getState().init({
          nodes: [
            { id: "n1", position: { x: i * 10, y: 0 }, data: {} },
          ],
          edges: [],
          viewport: { x: 0, y: 0, zoom: 1 },
        });
      });
      await act(async () => {
        await vi.advanceTimersByTimeAsync(200);
      });
    }

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1100);
    });

    expect(upsertCanvasLayout).toHaveBeenCalledTimes(1);
    expect(upsertCanvasLayout).toHaveBeenCalledWith(
      expect.objectContaining({
        nodes_json: expect.stringContaining('"x":40'),
      }),
    );
  });
});
