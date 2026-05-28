import { describe, it, expect, vi, beforeEach } from "vitest";
import { useCanvasStore } from "./canvasStore";
import {
  createAssetNode,
  createCharacterNode,
  createScriptNode,
  createStoryboardNode,
  createTaskNode,
} from "@/components/canvas/nodeFactory";

const linkShotSubject = vi.fn();
const unlinkShotSubject = vi.fn();
const toastError = vi.fn();

vi.mock("sonner", () => ({
  toast: {
    error: (msg: string) => toastError(msg),
    success: vi.fn(),
  },
}));
vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    linkShotSubject: (...args: unknown[]) => linkShotSubject(...args),
    unlinkShotSubject: (...args: unknown[]) => unlinkShotSubject(...args),
  },
}));

function seedFiveNodes() {
  const s = useCanvasStore.getState();
  s.addNode(createScriptNode("ep1"));
  s.addNode(createStoryboardNode("sh1"));
  s.addNode(createCharacterNode("c1"));
  s.addNode(createTaskNode("t1"));
  s.addNode(createAssetNode("a1"));
}

describe("canvasStore.onConnect", () => {
  beforeEach(() => {
    useCanvasStore.getState().reset();
    linkShotSubject.mockReset();
    unlinkShotSubject.mockReset();
    toastError.mockReset();
    linkShotSubject.mockResolvedValue({ status: "ok", data: null });
  });

  it("creates a character_to_shot edge and calls linkShotSubject", async () => {
    seedFiveNodes();
    useCanvasStore.getState().onConnect({
      source: "character-c1",
      target: "storyboard-sh1",
      sourceHandle: null,
      targetHandle: null,
    });
    expect(useCanvasStore.getState().edges).toHaveLength(1);
    expect(useCanvasStore.getState().edges[0]?.type).toBe("character_to_shot");
    await Promise.resolve();
    expect(linkShotSubject).toHaveBeenCalledWith("sh1", "c1", "character");
  });

  it("creates a shot_to_task edge with no IPC call", () => {
    seedFiveNodes();
    useCanvasStore.getState().onConnect({
      source: "storyboard-sh1",
      target: "task-t1",
      sourceHandle: null,
      targetHandle: null,
    });
    expect(useCanvasStore.getState().edges[0]?.type).toBe("shot_to_task");
    expect(linkShotSubject).not.toHaveBeenCalled();
  });

  it("creates a task_to_asset edge", () => {
    seedFiveNodes();
    useCanvasStore.getState().onConnect({
      source: "task-t1",
      target: "asset-a1",
      sourceHandle: null,
      targetHandle: null,
    });
    expect(useCanvasStore.getState().edges[0]?.type).toBe("task_to_asset");
  });

  it("rejects illegal connections with a toast and no edge", () => {
    seedFiveNodes();
    useCanvasStore.getState().onConnect({
      source: "character-c1",
      target: "task-t1",
      sourceHandle: null,
      targetHandle: null,
    });
    expect(useCanvasStore.getState().edges).toHaveLength(0);
    expect(toastError).toHaveBeenCalled();
  });

  it("rolls back the edge when linkShotSubject rejects", async () => {
    let reject: (err: unknown) => void = () => {};
    linkShotSubject.mockReturnValue(
      new Promise((_, rej) => {
        reject = rej;
      }),
    );
    seedFiveNodes();
    useCanvasStore.getState().onConnect({
      source: "character-c1",
      target: "storyboard-sh1",
      sourceHandle: null,
      targetHandle: null,
    });
    expect(useCanvasStore.getState().edges).toHaveLength(1);
    reject("boom");
    await Promise.resolve();
    await Promise.resolve();
    expect(useCanvasStore.getState().edges).toHaveLength(0);
    expect(toastError).toHaveBeenCalled();
  });
});
