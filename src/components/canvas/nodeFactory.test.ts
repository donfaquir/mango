import { describe, it, expect } from "vitest";
import {
  createScriptNode,
  createStoryboardNode,
  createCharacterNode,
  createTaskNode,
  createAssetNode,
} from "./nodeFactory";

describe("nodeFactory", () => {
  it("encodes the entity id into the node id with a kind prefix", () => {
    expect(createScriptNode("ep1").id).toBe("script-ep1");
    expect(createStoryboardNode("sh1").id).toBe("storyboard-sh1");
    expect(createCharacterNode("c1").id).toBe("character-c1");
    expect(createTaskNode("t1").id).toBe("task-t1");
    expect(createAssetNode("a1").id).toBe("asset-a1");
  });

  it("sets a matching `type` and `data.kind` on every factory output", () => {
    const checks: Array<[string, ReturnType<typeof createScriptNode>]> = [
      ["script", createScriptNode("ep1")],
      ["storyboard", createStoryboardNode("sh1") as never],
      ["character", createCharacterNode("c1") as never],
      ["task", createTaskNode("t1") as never],
      ["asset", createAssetNode("a1") as never],
    ];
    for (const [kind, node] of checks) {
      expect(node.type).toBe(kind);
      expect(node.data.kind).toBe(kind);
    }
  });

  it("places the node at the supplied position when given", () => {
    const node = createScriptNode("ep1", { x: 100, y: 200 });
    expect(node.position).toEqual({ x: 100, y: 200 });
  });
});
