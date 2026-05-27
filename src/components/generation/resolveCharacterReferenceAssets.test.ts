import { describe, expect, it, vi, beforeEach } from "vitest";
import type { Asset, Character } from "@/lib/bindings/commands";

const findAssetByPathMock = vi.fn<
  (
    projectId: string,
    filePath: string,
  ) => Promise<{ status: "ok"; data: Asset | null }>
>();

vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    findAssetByPath: (projectId: string, filePath: string) =>
      findAssetByPathMock(projectId, filePath),
  },
}));

import { resolveCharacterReferenceAssets } from "./resolveCharacterReferenceAssets";

function makeCharacter(overrides: Partial<Character> = {}): Character {
  return {
    id: "char-1",
    project_id: "proj-1",
    name: "小明",
    description: "",
    appearance_prompt: "",
    reference_image_path: "assets/abc.png",
    created_at: "2026-05-26 10:00:00",
    updated_at: "2026-05-26 10:00:00",
    ...overrides,
  };
}

function makeAsset(id: string, filePath: string): Asset {
  return {
    id,
    project_id: "proj-1",
    shot_id: null,
    asset_type: "image",
    original_name: "ref.png",
    file_path: filePath,
    thumbnail_path: null,
    file_size: 1024,
    content_hash: null,
    metadata_json: null,
    source: "imported",
    label: "",
    created_at: "2026-05-26 10:00:00",
    updated_at: "2026-05-26 10:00:00",
  };
}

describe("resolveCharacterReferenceAssets", () => {
  beforeEach(() => {
    findAssetByPathMock.mockReset();
  });

  it("translates each subject_id to the asset_id of its reference image", async () => {
    const characters = [
      makeCharacter({ id: "c1", reference_image_path: "assets/a.png" }),
      makeCharacter({
        id: "c2",
        name: "小红",
        reference_image_path: "assets/b.png",
      }),
    ];
    findAssetByPathMock.mockImplementation(async (_pid, filePath) => ({
      status: "ok",
      data:
        filePath === "assets/a.png"
          ? makeAsset("asset-1", filePath)
          : makeAsset("asset-2", filePath),
    }));

    const out = await resolveCharacterReferenceAssets(
      "proj-1",
      ["c1", "c2"],
      characters,
    );

    expect(out.errorMessage).toBeNull();
    expect(out.media).toEqual([
      { asset_id: "asset-1", type: "reference_image" },
      { asset_id: "asset-2", type: "reference_image" },
    ]);
    expect(findAssetByPathMock).toHaveBeenCalledTimes(2);
    expect(findAssetByPathMock).toHaveBeenCalledWith("proj-1", "assets/a.png");
  });

  it("returns errorMessage but no media when no subject can be resolved", async () => {
    const characters = [
      makeCharacter({ id: "c1", reference_image_path: "assets/missing.png" }),
    ];
    findAssetByPathMock.mockResolvedValue({ status: "ok", data: null });

    const out = await resolveCharacterReferenceAssets(
      "proj-1",
      ["c1"],
      characters,
    );

    expect(out.media).toEqual([]);
    expect(out.errorMessage).not.toBeNull();
    expect(out.errorMessage).toContain("小明");
    expect(out.errorMessage).toContain("参考图不在素材库");
  });

  it("emits a warning summary while keeping resolvable subjects in media[]", async () => {
    const characters = [
      makeCharacter({ id: "c1", reference_image_path: "assets/ok.png" }),
      makeCharacter({
        id: "c2",
        name: "小红",
        reference_image_path: "assets/ghost.png",
      }),
    ];
    findAssetByPathMock.mockImplementation(async (_pid, filePath) => ({
      status: "ok",
      data:
        filePath === "assets/ok.png" ? makeAsset("asset-1", filePath) : null,
    }));

    const out = await resolveCharacterReferenceAssets(
      "proj-1",
      ["c1", "c2"],
      characters,
    );

    expect(out.media).toEqual([
      { asset_id: "asset-1", type: "reference_image" },
    ]);
    expect(out.errorMessage).toContain("小红");
  });

  it("skips subjects whose character row has no reference_image_path", async () => {
    const characters = [
      makeCharacter({ id: "c1", reference_image_path: null, name: "无图" }),
    ];

    const out = await resolveCharacterReferenceAssets(
      "proj-1",
      ["c1"],
      characters,
    );

    expect(out.media).toEqual([]);
    expect(out.errorMessage).toContain("无图");
    expect(out.errorMessage).toContain("无参考图");
    expect(findAssetByPathMock).not.toHaveBeenCalled();
  });
});
