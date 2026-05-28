import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import { createElement, type ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const listEpisodes = vi.fn();
const createEpisode = vi.fn();
const reorderEpisodes = vi.fn();

vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    listEpisodes: (...args: unknown[]) => listEpisodes(...args),
    createEpisode: (...args: unknown[]) => createEpisode(...args),
    reorderEpisodes: (...args: unknown[]) => reorderEpisodes(...args),
  },
}));

import {
  useCreateEpisode,
  useEpisodeList,
  useReorderEpisodes,
} from "./useEpisodes";

function makeWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const wrapper = ({ children }: { children: ReactNode }) =>
    createElement(QueryClientProvider, { client: queryClient }, children);
  return { queryClient, wrapper };
}

describe("useEpisodeList", () => {
  beforeEach(() => {
    listEpisodes.mockReset();
    createEpisode.mockReset();
    reorderEpisodes.mockReset();
  });

  it("fetches episodes scoped to the given project_id", async () => {
    listEpisodes.mockResolvedValueOnce({
      status: "ok",
      data: [
        {
          id: "e1",
          project_id: "p1",
          title: "Ep 1",
          order_index: 0,
          script_text: "",
          created_at: "",
          updated_at: "",
        },
      ],
    });

    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useEpisodeList("p1"), { wrapper });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data?.[0]?.id).toBe("e1");
    expect(listEpisodes).toHaveBeenCalledWith({ project_id: "p1" });
  });

  it("is disabled when projectId is undefined", () => {
    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useEpisodeList(undefined), {
      wrapper,
    });
    expect(result.current.fetchStatus).toBe("idle");
    expect(listEpisodes).not.toHaveBeenCalled();
  });
});

describe("useCreateEpisode", () => {
  beforeEach(() => {
    listEpisodes.mockReset();
    createEpisode.mockReset();
  });

  it("invalidates episode list scoped to the input project_id", async () => {
    createEpisode.mockResolvedValueOnce({
      status: "ok",
      data: {
        id: "e1",
        project_id: "p1",
        title: "Ep 1",
        order_index: 0,
        script_text: "",
        created_at: "",
        updated_at: "",
      },
    });

    const { queryClient, wrapper } = makeWrapper();
    queryClient.setQueryData(["episodes", "p1"], []);
    queryClient.setQueryData(["episodes", "p2"], []);

    const { result } = renderHook(() => useCreateEpisode(), { wrapper });
    await result.current.mutateAsync({
      project_id: "p1",
      title: "Ep 1",
      script_text: null,
    });

    expect(
      queryClient.getQueryState(["episodes", "p1"])?.isInvalidated,
    ).toBe(true);
    expect(
      queryClient.getQueryState(["episodes", "p2"])?.isInvalidated,
    ).toBe(false);
  });
});

describe("useReorderEpisodes", () => {
  beforeEach(() => {
    reorderEpisodes.mockReset();
  });

  it("forwards projectId and ordered ids and invalidates the list", async () => {
    reorderEpisodes.mockResolvedValueOnce({ status: "ok", data: null });
    const { queryClient, wrapper } = makeWrapper();
    queryClient.setQueryData(["episodes", "p1"], []);

    const { result } = renderHook(() => useReorderEpisodes("p1"), { wrapper });
    await result.current.mutateAsync(["e3", "e1", "e2"]);

    expect(reorderEpisodes).toHaveBeenCalledWith("p1", ["e3", "e1", "e2"]);
    expect(
      queryClient.getQueryState(["episodes", "p1"])?.isInvalidated,
    ).toBe(true);
  });
});
