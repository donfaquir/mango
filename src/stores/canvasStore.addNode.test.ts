import { describe, it, expect, vi, beforeEach } from "vitest";
import { useCanvasStore } from "./canvasStore";
import {
  createCharacterNode,
  createScriptNode,
} from "@/components/canvas/nodeFactory";

vi.mock("sonner", () => ({
  toast: {
    error: vi.fn(),
    success: vi.fn(),
  },
}));
vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    linkShotSubject: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    unlinkShotSubject: vi.fn().mockResolvedValue({ status: "ok", data: null }),
  },
}));

describe("canvasStore.addNode / updateNodeData", () => {
  beforeEach(() => {
    useCanvasStore.getState().reset();
  });

  it("appends a node when its id is unique", () => {
    const node = createScriptNode("ep1");
    useCanvasStore.getState().addNode(node);
    expect(useCanvasStore.getState().nodes).toHaveLength(1);
    expect(useCanvasStore.getState().nodes[0]?.id).toBe("script-ep1");
  });

  it("rejects duplicate ids without mutating state", async () => {
    const { toast } = await import("sonner");
    const node = createScriptNode("ep1");
    useCanvasStore.getState().addNode(node);
    useCanvasStore.getState().addNode(node);
    expect(useCanvasStore.getState().nodes).toHaveLength(1);
    expect(toast.error).toHaveBeenCalledWith("该节点已在画布上");
  });

  it("merges partial data via updateNodeData without losing other fields", () => {
    useCanvasStore.getState().addNode(createCharacterNode("c1"));
    useCanvasStore
      .getState()
      .updateNodeData("character-c1", { characterId: "c2" });
    const stored = useCanvasStore
      .getState()
      .nodes.find((n) => n.id === "character-c1");
    expect(stored?.data.kind).toBe("character");
    expect((stored?.data as { characterId: string }).characterId).toBe("c2");
  });
});
