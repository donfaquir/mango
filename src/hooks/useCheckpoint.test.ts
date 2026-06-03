import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import { createElement, type ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const createEpisodeCheckpoint = vi.fn();

vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    createEpisodeCheckpoint: (...args: unknown[]) =>
      createEpisodeCheckpoint(...args),
    listEpisodeCheckpoints: vi.fn(),
    restoreEpisodeCheckpoint: vi.fn(),
    deleteEpisodeCheckpoint: vi.fn(),
  },
  events: {
    episodeDataRestored: { listen: vi.fn(() => Promise.resolve(() => {})) },
  },
}));

import { useCreateCheckpoint } from "./useCheckpoint";

function makeWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const wrapper = ({ children }: { children: ReactNode }) =>
    createElement(QueryClientProvider, { client: queryClient }, children);
  return { queryClient, wrapper };
}

describe("useCreateCheckpoint", () => {
  beforeEach(() => {
    createEpisodeCheckpoint.mockReset();
  });

  it("forwards the input and unwraps the ok branch", async () => {
    createEpisodeCheckpoint.mockResolvedValueOnce({
      status: "ok",
      data: {
        id: "cp1",
        episode_id: "ep1",
        version_number: 1,
        label: null,
        trigger_type: "manual",
        script_text: "",
        shots_json: "[]",
        canvas_nodes_json: "[]",
        canvas_edges_json: "[]",
        canvas_viewport_json: '{"x":0,"y":0,"zoom":1}',
        change_summary: null,
        created_at: "2026-05-29T00:00:00Z",
      },
    });

    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useCreateCheckpoint(), { wrapper });

    const checkpoint = await result.current.mutateAsync({
      episode_id: "ep1",
      label: null,
    });
    expect(createEpisodeCheckpoint).toHaveBeenCalledWith({
      episode_id: "ep1",
      label: null,
    });
    expect(checkpoint.id).toBe("cp1");
    expect(checkpoint.version_number).toBe(1);
  });

  it("throws an IpcCallError on the error branch", async () => {
    createEpisodeCheckpoint.mockResolvedValueOnce({
      status: "error",
      error: { code: "NotFound", message: "no layout for ep1" },
    });

    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useCreateCheckpoint(), { wrapper });

    await expect(
      result.current.mutateAsync({ episode_id: "ep1", label: null }),
    ).rejects.toMatchObject({
      name: "IpcCallError",
      code: "NotFound",
    });
    await waitFor(() => expect(result.current.isError).toBe(true));
  });
});
