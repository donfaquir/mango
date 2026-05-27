import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import { createElement, type ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const listAssets = vi.fn();
const importAsset = vi.fn();
const updateAssetLabel = vi.fn();

vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    listAssets: (...args: unknown[]) => listAssets(...args),
    importAsset: (...args: unknown[]) => importAsset(...args),
    updateAssetLabel: (...args: unknown[]) => updateAssetLabel(...args),
  },
}));

import { useAssetList, useImportAsset } from "./useAssets";

function makeWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const wrapper = ({ children }: { children: ReactNode }) =>
    createElement(QueryClientProvider, { client: queryClient }, children);
  return { queryClient, wrapper };
}

const sampleAsset = {
  id: "a1",
  project_id: "p1",
  shot_id: null,
  asset_type: "image" as const,
  original_name: "x.png",
  file_path: "assets/a1.png",
  thumbnail_path: "thumbnails/a1_thumb.webp",
  file_size: 0,
  content_hash: "h",
  metadata_json: null,
  source: "imported" as const,
  label: "",
  created_at: "",
  updated_at: "",
};

describe("useAssetList", () => {
  beforeEach(() => {
    listAssets.mockReset();
    importAsset.mockReset();
  });

  it("fetches assets scoped to project and type filter", async () => {
    listAssets.mockResolvedValueOnce({ status: "ok", data: [sampleAsset] });

    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useAssetList("p1", "image"), {
      wrapper,
    });

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data?.[0]?.id).toBe("a1");
    expect(listAssets).toHaveBeenCalledWith({
      project_id: "p1",
      asset_type: "image",
      source: null,
      keyword: null,
      limit: null,
      offset: null,
    });
  });

  it("is disabled when projectId is undefined", () => {
    const { wrapper } = makeWrapper();
    const { result } = renderHook(() => useAssetList(undefined), { wrapper });
    expect(result.current.fetchStatus).toBe("idle");
    expect(listAssets).not.toHaveBeenCalled();
  });
});

describe("useImportAsset", () => {
  beforeEach(() => {
    listAssets.mockReset();
    importAsset.mockReset();
  });

  it("invalidates the assets list for the imported project", async () => {
    importAsset.mockResolvedValueOnce({ status: "ok", data: sampleAsset });
    const { queryClient, wrapper } = makeWrapper();

    queryClient.setQueryData(["assets", "p1", "all"], [sampleAsset]);
    queryClient.setQueryData(["assets", "p2", "all"], []);

    const { result } = renderHook(() => useImportAsset(), { wrapper });
    await result.current.mutateAsync({
      project_id: "p1",
      source_path: "/tmp/x.png",
    });

    const p1 = queryClient.getQueryState(["assets", "p1", "all"]);
    const p2 = queryClient.getQueryState(["assets", "p2", "all"]);
    expect(p1?.isInvalidated).toBe(true);
    expect(p2?.isInvalidated).toBe(false);
  });
});
