import { describe, it, expect, vi, beforeEach } from "vitest";
import { useCanvasStore } from "./canvasStore";
import { createScriptNode } from "@/components/canvas/nodeFactory";

vi.mock("sonner", () => ({
  toast: { error: vi.fn(), success: vi.fn() },
}));
vi.mock("@/lib/bindings/commands", () => ({
  commands: {
    linkShotSubject: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    unlinkShotSubject: vi.fn().mockResolvedValue({ status: "ok", data: null }),
  },
}));

function temporal() {
  return useCanvasStore.temporal.getState();
}

describe("canvasStore temporal stack", () => {
  beforeEach(() => {
    useCanvasStore.getState().reset();
  });

  it("addNode pushes a snapshot that undo can roll back", () => {
    useCanvasStore.getState().addNode(createScriptNode("ep1"));
    expect(useCanvasStore.getState().nodes).toHaveLength(1);
    expect(temporal().pastStates.length).toBeGreaterThan(0);

    temporal().undo();
    expect(useCanvasStore.getState().nodes).toHaveLength(0);
    expect(temporal().futureStates.length).toBeGreaterThan(0);
  });

  it("redo restores the node after undo", () => {
    useCanvasStore.getState().addNode(createScriptNode("ep1"));
    temporal().undo();
    temporal().redo();
    expect(useCanvasStore.getState().nodes).toHaveLength(1);
  });

  it("viewport pan/zoom does not enter the undo stack", () => {
    useCanvasStore.getState().addNode(createScriptNode("ep1"));
    const before = temporal().pastStates.length;
    useCanvasStore.getState().setViewport({ x: 100, y: 200, zoom: 1.5 });
    useCanvasStore.getState().setViewport({ x: -50, y: 0, zoom: 0.8 });
    expect(temporal().pastStates.length).toBe(before);
  });

  it("init() does not pollute the undo stack", () => {
    useCanvasStore.getState().init({
      nodes: [createScriptNode("ep1")],
      edges: [],
      viewport: { x: 0, y: 0, zoom: 1 },
    });
    expect(temporal().pastStates).toHaveLength(0);
    expect(temporal().futureStates).toHaveLength(0);
  });

  it("init() clears stack from a previous episode", () => {
    useCanvasStore.getState().addNode(createScriptNode("ep1"));
    expect(temporal().pastStates.length).toBeGreaterThan(0);
    useCanvasStore.getState().init({
      nodes: [],
      edges: [],
      viewport: { x: 0, y: 0, zoom: 1 },
    });
    expect(temporal().pastStates).toHaveLength(0);
  });

  it("reset() clears nodes and the undo stack", () => {
    useCanvasStore.getState().addNode(createScriptNode("ep1"));
    useCanvasStore.getState().reset();
    expect(useCanvasStore.getState().nodes).toHaveLength(0);
    expect(temporal().pastStates).toHaveLength(0);
    expect(temporal().futureStates).toHaveLength(0);
  });

  it("caps the undo stack at 50 entries", () => {
    for (let i = 0; i < 60; i++) {
      useCanvasStore.getState().addNode(createScriptNode(`ep${i}`));
    }
    expect(temporal().pastStates.length).toBeLessThanOrEqual(50);
  });
});
