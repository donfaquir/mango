import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import { createElement, type ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const listShots = vi.fn();
const linkShotSubject = vi.fn();
const unlinkShotSubject = vi.fn();

vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    listShots: (...args: unknown[]) => listShots(...args),
    linkShotSubject: (...args: unknown[]) => linkShotSubject(...args),
    unlinkShotSubject: (...args: unknown[]) => unlinkShotSubject(...args),
  },
}));

import {
  useLinkShotSubject,
  useShotList,
  useUnlinkShotSubject,
} from "./useShots";

function makeWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const wrapper = ({ children }: { children: ReactNode }) =>
    createElement(QueryClientProvider, { client: queryClient }, children);
  return { queryClient, wrapper };
}

describe("useShotList", () => {
  beforeEach(() => {
    listShots.mockReset();
  });

  it("fetches shots scoped to the given episode_id", async () => {
    listShots.mockResolvedValueOnce({
      status: "ok",
      data: [],
    });

    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useShotList("e1"), { wrapper });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(listShots).toHaveBeenCalledWith({ episode_id: "e1" });
  });
});

describe("useLinkShotSubject", () => {
  beforeEach(() => {
    linkShotSubject.mockReset();
    unlinkShotSubject.mockReset();
  });

  it("forwards args and invalidates the shot links cache", async () => {
    linkShotSubject.mockResolvedValueOnce({ status: "ok", data: null });
    const { queryClient, wrapper } = makeWrapper();
    queryClient.setQueryData(["shot", "s1", "links"], {});

    const { result } = renderHook(() => useLinkShotSubject(), { wrapper });
    await result.current.mutateAsync({
      shotId: "s1",
      subjectId: "c1",
      subjectKind: "character",
    });

    expect(linkShotSubject).toHaveBeenCalledWith("s1", "c1", "character");
    expect(
      queryClient.getQueryState(["shot", "s1", "links"])?.isInvalidated,
    ).toBe(true);
  });

  it("unlink uses the same dispatch shape", async () => {
    unlinkShotSubject.mockResolvedValueOnce({ status: "ok", data: null });
    const { wrapper } = makeWrapper();

    const { result } = renderHook(() => useUnlinkShotSubject(), { wrapper });
    await result.current.mutateAsync({
      shotId: "s1",
      subjectId: "sc1",
      subjectKind: "scene",
    });

    expect(unlinkShotSubject).toHaveBeenCalledWith("s1", "sc1", "scene");
  });
});
