import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import { createElement, type ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const listVideoClips = vi.fn();
const createVideoClip = vi.fn();
const deleteVideoClip = vi.fn();
const reorderVideoClips = vi.fn();

vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    listVideoClips: (...args: unknown[]) => listVideoClips(...args),
    createVideoClip: (...args: unknown[]) => createVideoClip(...args),
    deleteVideoClip: (...args: unknown[]) => deleteVideoClip(...args),
    reorderVideoClips: (...args: unknown[]) => reorderVideoClips(...args),
  },
}));

vi.mock("sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

import {
  useVideoClipList,
  useCreateVideoClip,
  useDeleteVideoClip,
  useReorderVideoClips,
} from "./useVideoClips";

function makeWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const wrapper = ({ children }: { children: ReactNode }) =>
    createElement(QueryClientProvider, { client: queryClient }, children);
  return { queryClient, wrapper };
}

describe("useVideoClipList", () => {
  beforeEach(() => {
    listVideoClips.mockReset();
  });

  it("fetches clips scoped to the given episode_id", async () => {
    listVideoClips.mockResolvedValueOnce({ status: "ok", data: [] });
    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useVideoClipList("e1"), { wrapper });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(listVideoClips).toHaveBeenCalledWith("e1");
  });
});

describe("useCreateVideoClip", () => {
  beforeEach(() => {
    createVideoClip.mockReset();
  });

  it("creates a clip and invalidates the cache", async () => {
    const clip = {
      id: "c1",
      project_id: "p1",
      episode_id: "e1",
      source_asset_id: "a1",
      label: null,
      trim_start_ms: null,
      trim_end_ms: null,
      order_index: 0,
      created_at: "",
    };
    createVideoClip.mockResolvedValueOnce({ status: "ok", data: clip });
    const { queryClient, wrapper } = makeWrapper();
    queryClient.setQueryData(["videoClips", "e1"], []);

    const { result } = renderHook(() => useCreateVideoClip("e1"), { wrapper });
    await result.current.mutateAsync({
      project_id: "p1",
      source_asset_id: "a1",
      episode_id: "e1",
      label: null,
      trim_start_ms: null,
      trim_end_ms: null,
    });

    expect(createVideoClip).toHaveBeenCalled();
    expect(
      queryClient.getQueryState(["videoClips", "e1"])?.isInvalidated,
    ).toBe(true);
  });
});

describe("useDeleteVideoClip", () => {
  beforeEach(() => {
    deleteVideoClip.mockReset();
  });

  it("deletes a clip and invalidates the cache", async () => {
    deleteVideoClip.mockResolvedValueOnce({ status: "ok", data: null });
    const { queryClient, wrapper } = makeWrapper();
    queryClient.setQueryData(["videoClips", "e1"], []);

    const { result } = renderHook(() => useDeleteVideoClip("e1"), { wrapper });
    await result.current.mutateAsync("c1");

    expect(deleteVideoClip).toHaveBeenCalledWith("c1");
    expect(
      queryClient.getQueryState(["videoClips", "e1"])?.isInvalidated,
    ).toBe(true);
  });
});

describe("useReorderVideoClips", () => {
  beforeEach(() => {
    reorderVideoClips.mockReset();
  });

  it("reorders clips and invalidates the cache", async () => {
    reorderVideoClips.mockResolvedValueOnce({ status: "ok", data: null });
    const { queryClient, wrapper } = makeWrapper();
    queryClient.setQueryData(["videoClips", "e1"], []);

    const { result } = renderHook(() => useReorderVideoClips("e1"), {
      wrapper,
    });
    await result.current.mutateAsync(["c2", "c1", "c3"]);

    expect(reorderVideoClips).toHaveBeenCalledWith(["c2", "c1", "c3"]);
    expect(
      queryClient.getQueryState(["videoClips", "e1"])?.isInvalidated,
    ).toBe(true);
  });
});
