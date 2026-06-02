import { describe, it, expect, vi, beforeEach } from "vitest";
import { useCanvasStore } from "./canvasStore";
import {
  createAssetNode,
  createCharacterNode,
  createStoryboardNode,
  createTaskNode,
} from "@/components/canvas/nodeFactory";

const linkShotSubject = vi.fn();
const unlinkShotSubject = vi.fn();

vi.mock("sonner", () => ({
  toast: {
    error: vi.fn(),
    success: vi.fn(),
  },
}));
vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    linkShotSubject: (...args: unknown[]) => linkShotSubject(...args),
    unlinkShotSubject: (...args: unknown[]) => unlinkShotSubject(...args),
  },
}));

describe("canvasStore reverse-unlink on delete", () => {
  beforeEach(() => {
    useCanvasStore.getState().reset();
    linkShotSubject.mockReset();
    unlinkShotSubject.mockReset();
    linkShotSubject.mockResolvedValue({ status: "ok", data: null });
    unlinkShotSubject.mockResolvedValue({ status: "ok", data: null });
  });

  function setupCharacterShotEdge() {
    const s = useCanvasStore.getState();
    s.addNode(createStoryboardNode("sh1"));
    s.addNode(createCharacterNode("c1"));
    s.onConnect({
      source: "character-c1",
      target: "storyboard-sh1",
      sourceHandle: null,
      targetHandle: null,
    });
    linkShotSubject.mockClear();
  }

  it("unlinks when the character node is removed", () => {
    setupCharacterShotEdge();
    useCanvasStore.getState().removeNode("character-c1");
    expect(unlinkShotSubject).toHaveBeenCalledWith("sh1", "c1", "character");
  });

  it("unlinks when the storyboard node is removed", () => {
    setupCharacterShotEdge();
    useCanvasStore.getState().removeNode("storyboard-sh1");
    expect(unlinkShotSubject).toHaveBeenCalledWith("sh1", "c1", "character");
  });

  it("unlinks when the character_to_shot edge is removed directly", () => {
    setupCharacterShotEdge();
    const edgeId = useCanvasStore.getState().edges[0]!.id;
    useCanvasStore.getState().removeEdge(edgeId);
    expect(unlinkShotSubject).toHaveBeenCalledWith("sh1", "c1", "character");
  });

  it("does NOT trigger unlinkShotSubject for other edge kinds", () => {
    const s = useCanvasStore.getState();
    s.addNode(createTaskNode("t1"));
    s.addNode(createAssetNode("a1"));
    s.onConnect({
      source: "task-t1",
      target: "asset-a1",
      sourceHandle: null,
      targetHandle: null,
    });
    const edgeId = useCanvasStore.getState().edges[0]!.id;
    useCanvasStore.getState().removeEdge(edgeId);
    expect(unlinkShotSubject).not.toHaveBeenCalled();
  });
});
