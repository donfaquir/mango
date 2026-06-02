import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, within } from "@testing-library/react";
import type { Asset } from "@/lib/bindings/commands";

const useAssetListMock = vi.fn();
const useAssetLabelsMock = vi.fn();
vi.mock("@/hooks/useAssets", () => ({
  useAssetList: (...args: unknown[]) => useAssetListMock(...args),
  useAssetLabels: (...args: unknown[]) => useAssetLabelsMock(...args),
}));

vi.mock("@/hooks/useResolvedAssetUrl", () => ({
  useResolvedAssetUrl: () => "asset://fake",
}));

import { AssetDrawer } from "./AssetDrawer";
import { DRAG_MIME_ASSET_ID } from "./constants";

function makeAsset(id: string, name: string): Asset {
  return {
    id,
    project_id: "p1",
    shot_id: null,
    asset_type: "image",
    original_name: name,
    file_path: `assets/${id}.png`,
    thumbnail_path: `thumbnails/${id}_thumb.webp`,
    file_size: 0,
    content_hash: "h",
    metadata_json: null,
    source: "imported",
    label: "",
    created_at: "",
    updated_at: "",
  };
}

const assets: Asset[] = [
  makeAsset("a1", "one.png"),
  makeAsset("a2", "two.png"),
];

beforeEach(() => {
  useAssetListMock.mockReset();
  useAssetListMock.mockReturnValue({ data: assets, isLoading: false });
  useAssetLabelsMock.mockReset();
  useAssetLabelsMock.mockReturnValue({ data: [], isLoading: false });
});

describe("AssetDrawer", () => {
  it("starts collapsed (translated off-screen)", () => {
    const { container } = render(
      <AssetDrawer projectId="p1" projectRoot="/tmp" />,
    );
    const aside = container.querySelector("aside");
    expect(aside?.className).toContain("translate-x-full");
    expect(aside?.getAttribute("aria-hidden")).toBe("true");
  });

  it("toggles open when the trigger button is clicked", () => {
    const { container } = render(
      <AssetDrawer projectId="p1" projectRoot="/tmp" />,
    );
    fireEvent.click(screen.getByLabelText("切换素材库抽屉"));
    const aside = container.querySelector("aside");
    expect(aside?.className).toContain("translate-x-0");
    expect(aside?.getAttribute("aria-hidden")).toBe("false");
  });

  it("closes when the close button is clicked", () => {
    const { container } = render(
      <AssetDrawer projectId="p1" projectRoot="/tmp" />,
    );
    fireEvent.click(screen.getByLabelText("切换素材库抽屉"));
    fireEvent.click(screen.getByLabelText("关闭素材库抽屉"));
    const aside = container.querySelector("aside");
    expect(aside?.className).toContain("translate-x-full");
  });

  it("renders one draggable wrapper per asset returned by useAssetList", () => {
    const { container } = render(
      <AssetDrawer projectId="p1" projectRoot="/tmp" />,
    );
    fireEvent.click(screen.getByLabelText("切换素材库抽屉"));
    const aside = container.querySelector("aside")!;
    const draggables = within(aside).getAllByText(/\.png$/);
    expect(draggables).toHaveLength(2);
  });

  it("disables native drag on inner thumbnails so the wrapper drag wins", () => {
    const { container } = render(
      <AssetDrawer projectId="p1" projectRoot="/tmp" />,
    );
    fireEvent.click(screen.getByLabelText("切换素材库抽屉"));
    const aside = container.querySelector("aside")!;
    const imgs = aside.querySelectorAll("img");
    expect(imgs.length).toBeGreaterThan(0);
    for (const img of imgs) {
      expect(img.getAttribute("draggable")).toBe("false");
    }
  });

  it("sets the asset id on dataTransfer when drag begins", () => {
    const { container } = render(
      <AssetDrawer projectId="p1" projectRoot="/tmp" />,
    );
    fireEvent.click(screen.getByLabelText("切换素材库抽屉"));

    const aside = container.querySelector("aside")!;
    const wrappers = aside.querySelectorAll("[draggable='true']");
    expect(wrappers.length).toBe(2);

    const setData = vi.fn();
    const dataTransfer = {
      setData,
      get effectAllowed() {
        return "none";
      },
      set effectAllowed(_v: string) {},
    };

    fireEvent.dragStart(wrappers[0]!, { dataTransfer });
    expect(setData).toHaveBeenCalledWith(DRAG_MIME_ASSET_ID, "a1");
  });

  it("passes the filter values into useAssetList", () => {
    render(<AssetDrawer projectId="p1" projectRoot="/tmp" />);
    expect(useAssetListMock).toHaveBeenCalledWith("p1", undefined, {
      source: undefined,
      keyword: undefined,
      label: undefined,
    });
  });
});
