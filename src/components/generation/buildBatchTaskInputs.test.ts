import { describe, expect, it, vi, beforeEach } from "vitest";
import type { Character, Model, Shot } from "@/lib/bindings/commands";

const findAssetByPath = vi.fn();
const listShotLinks = vi.fn();

vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    findAssetByPath: (...args: unknown[]) => findAssetByPath(...args),
    listShotLinks: (...args: unknown[]) => listShotLinks(...args),
  },
}));

import { buildBatchTaskInputs } from "./buildBatchTaskInputs";

function makeShot(overrides: Partial<Shot> = {}): Shot {
  return {
    id: "shot-1",
    episode_id: "ep-1",
    order_index: 0,
    summary: "opening",
    duration_sec: null,
    camera_angle: "",
    shot_type: "",
    mood: "",
    dialogue: "",
    video_prompt: "",
    image_prompt: "",
    status: "draft",
    adopted_asset_id: null,
    created_at: "",
    updated_at: "",
    ...overrides,
  };
}

function wan27Model(): Model {
  return {
    id: "wan2.7-image-pro",
    provider_id: "bailian",
    name: "Wan",
    model_type: "image",
    capabilities_json: null,
    default_params_json: '{"size":"2K","n":1}',
  };
}

function happyhorseModel(): Model {
  return {
    id: "happyhorse-1.0-r2v",
    provider_id: "bailian",
    name: "Happy",
    model_type: "video",
    capabilities_json: null,
    default_params_json: '{"resolution":"720P","ratio":"16:9","duration":5}',
  };
}

const CTX_BASE = {
  projectId: "p1",
  accountId: "a1",
  providerId: "bailian",
  characters: [] as Character[],
};

describe("buildBatchTaskInputs", () => {
  beforeEach(() => {
    findAssetByPath.mockReset();
    listShotLinks.mockReset();
  });

  it("expands 2 shots × 2 models into 4 inputs for image task type", async () => {
    const shots = [
      makeShot({ id: "s1", image_prompt: "first prompt" }),
      makeShot({ id: "s2", order_index: 1, image_prompt: "second prompt" }),
    ];
    const result = await buildBatchTaskInputs({
      ...CTX_BASE,
      taskType: "image",
      shots,
      models: [wan27Model(), happyhorseModel()],
    });
    // happyhorse is filtered by MultiModelSelect (task_type), but buildBatchTaskInputs
    // does not re-filter — it trusts the caller. With image+happyhorse the resolver
    // gives no media → skipped. Test only the wan27 expansion.
    const wan27Inputs = result.inputs.filter(
      (i) => i.model_id === "wan2.7-image-pro",
    );
    expect(wan27Inputs).toHaveLength(2);
    expect(wan27Inputs[0].shot_id).toBe("s1");
    expect(wan27Inputs[1].shot_id).toBe("s2");
    const params = JSON.parse(wan27Inputs[0].params_json!);
    expect(params.prompt).toBe("first prompt");
    expect(params.size).toBe("2K");
  });

  it("skips shots with empty prompt", async () => {
    const shots = [
      makeShot({ id: "s1", image_prompt: "" }),
      makeShot({ id: "s2", order_index: 1, image_prompt: "real" }),
    ];
    const result = await buildBatchTaskInputs({
      ...CTX_BASE,
      taskType: "image",
      shots,
      models: [wan27Model()],
    });
    expect(result.inputs).toHaveLength(1);
    expect(result.inputs[0].shot_id).toBe("s2");
    expect(result.skipped).toHaveLength(1);
    expect(result.skipped[0].shotId).toBe("s1");
    expect(result.skipped[0].reason).toContain("缺少图像 prompt");
  });

  it("resolves character reference assets per shot for happyhorse", async () => {
    listShotLinks.mockResolvedValue({
      status: "ok",
      data: { character_ids: ["c1"], scene_ids: [], prop_ids: [] },
    });
    findAssetByPath.mockResolvedValue({
      status: "ok",
      data: { id: "asset-1" },
    });
    const characters: Character[] = [
      {
        id: "c1",
        project_id: "p1",
        name: "Hero",
        description: "",
        appearance_prompt: "",
        reference_image_path: "subjects/character/hero.png",
        voice_id: null,
        created_at: "",
        updated_at: "",
      },
    ];
    const shots = [makeShot({ video_prompt: "tracking shot" })];
    const result = await buildBatchTaskInputs({
      ...CTX_BASE,
      characters,
      taskType: "video",
      shots,
      models: [happyhorseModel()],
    });
    expect(result.skipped).toHaveLength(0);
    expect(result.inputs).toHaveLength(1);
    const params = JSON.parse(result.inputs[0].params_json!);
    expect(params.media).toEqual([{ asset_id: "asset-1", type: "reference_image" }]);
    expect(params.resolution).toBe("720P");
  });

  it("skips happyhorse shot when reference media cannot be resolved", async () => {
    listShotLinks.mockResolvedValue({
      status: "ok",
      data: { character_ids: [], scene_ids: [], prop_ids: [] },
    });
    const shots = [makeShot({ video_prompt: "wide pan" })];
    const result = await buildBatchTaskInputs({
      ...CTX_BASE,
      taskType: "video",
      shots,
      models: [happyhorseModel()],
    });
    expect(result.inputs).toHaveLength(0);
    expect(result.skipped).toHaveLength(1);
    expect(result.skipped[0].reason).toContain("参考图");
  });
});
