import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import { createElement, type ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const listCharacters = vi.fn();
const deleteCharacter = vi.fn();

vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    listCharacters: (...args: unknown[]) => listCharacters(...args),
    deleteCharacter: (...args: unknown[]) => deleteCharacter(...args),
  },
}));

import { useCharacterList, useDeleteCharacter } from "./useCharacters";

function makeWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const wrapper = ({ children }: { children: ReactNode }) =>
    createElement(QueryClientProvider, { client: queryClient }, children);
  return { queryClient, wrapper };
}

describe("useCharacterList", () => {
  beforeEach(() => {
    listCharacters.mockReset();
    deleteCharacter.mockReset();
  });

  it("fetches characters scoped to the given project_id", async () => {
    listCharacters.mockResolvedValueOnce({
      status: "ok",
      data: [
        {
          id: "c1",
          project_id: "p1",
          name: "Alice",
          description: "",
          appearance_prompt: "",
          reference_image_path: null,
          created_at: "",
          updated_at: "",
        },
      ],
    });

    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useCharacterList("p1"), { wrapper });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data?.[0]?.id).toBe("c1");
    expect(listCharacters).toHaveBeenCalledWith({ project_id: "p1" });
  });

  it("is disabled when projectId is undefined", () => {
    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useCharacterList(undefined), {
      wrapper,
    });
    expect(result.current.fetchStatus).toBe("idle");
    expect(listCharacters).not.toHaveBeenCalled();
  });
});

describe("useDeleteCharacter", () => {
  beforeEach(() => {
    listCharacters.mockReset();
    deleteCharacter.mockReset();
  });

  it("invalidates characters and costumes scoped to the given project", async () => {
    deleteCharacter.mockResolvedValueOnce({ status: "ok", data: null });
    const { queryClient, wrapper } = makeWrapper();

    // Seed two stale queries (target project) and one out-of-scope query.
    queryClient.setQueryData(["characters", "p1"], ["a"]);
    queryClient.setQueryData(["costumes", "p1"], ["x"]);
    queryClient.setQueryData(["characters", "p2"], ["unrelated"]);

    const { result } = renderHook(() => useDeleteCharacter("p1"), { wrapper });
    await result.current.mutateAsync("c1");

    const charsP1 = queryClient.getQueryState(["characters", "p1"]);
    const costumesP1 = queryClient.getQueryState(["costumes", "p1"]);
    const charsP2 = queryClient.getQueryState(["characters", "p2"]);

    expect(charsP1?.isInvalidated).toBe(true);
    expect(costumesP1?.isInvalidated).toBe(true);
    expect(charsP2?.isInvalidated).toBe(false);
  });
});
