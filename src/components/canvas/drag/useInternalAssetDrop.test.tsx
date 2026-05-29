import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";

const screenToFlowPosition = vi.fn();
const getIntersectingNodes = vi.fn();

vi.mock("@xyflow/react", () => ({
  useReactFlow: () => ({ screenToFlowPosition, getIntersectingNodes }),
}));

import { useInternalAssetDrop } from "./useInternalAssetDrop";
import { DRAG_MIME_ASSET_ID } from "./constants";

interface FakeDataTransfer {
  data: Record<string, string>;
  types: string[];
  effectAllowed: string;
  dropEffect: string;
  getData: (k: string) => string;
  setData: (k: string, v: string) => void;
}

function makeDataTransfer(types: string[], data: Record<string, string> = {}): FakeDataTransfer {
  return {
    data,
    types,
    effectAllowed: "none",
    dropEffect: "none",
    getData(k: string) {
      return this.data[k] ?? "";
    },
    setData(k: string, v: string) {
      this.data[k] = v;
    },
  };
}

function dragEvent(
  dataTransfer: FakeDataTransfer,
  client: { x: number; y: number } = { x: 0, y: 0 },
  target?: EventTarget,
): React.DragEvent {
  const t = target ?? ({} as EventTarget);
  return {
    dataTransfer,
    clientX: client.x,
    clientY: client.y,
    currentTarget: t,
    target: t,
    preventDefault: vi.fn(),
  } as unknown as React.DragEvent;
}

beforeEach(() => {
  screenToFlowPosition.mockReset();
  getIntersectingNodes.mockReset();
  screenToFlowPosition.mockImplementation(({ x, y }: { x: number; y: number }) => ({
    x,
    y,
  }));
});

describe("useInternalAssetDrop", () => {
  it("sets isOver and dropEffect when the asset MIME is present", () => {
    const onPlaceAsset = vi.fn();
    const onConnectAsset = vi.fn();
    const { result } = renderHook(() =>
      useInternalAssetDrop({ onPlaceAsset, onConnectAsset }),
    );

    const dt = makeDataTransfer([DRAG_MIME_ASSET_ID]);
    const ev = dragEvent(dt);
    act(() => result.current.onDragOver(ev));

    expect(ev.preventDefault).toHaveBeenCalled();
    expect(dt.dropEffect).toBe("copy");
    expect(result.current.isOver).toBe(true);
  });

  it("ignores onDragOver when the asset MIME is absent", () => {
    const onPlaceAsset = vi.fn();
    const onConnectAsset = vi.fn();
    const { result } = renderHook(() =>
      useInternalAssetDrop({ onPlaceAsset, onConnectAsset }),
    );

    const dt = makeDataTransfer(["text/plain"]);
    const ev = dragEvent(dt);
    act(() => result.current.onDragOver(ev));

    expect(ev.preventDefault).not.toHaveBeenCalled();
    expect(dt.dropEffect).toBe("none");
    expect(result.current.isOver).toBe(false);
  });

  it("calls onPlaceAsset with the flow position when no node is hit", () => {
    getIntersectingNodes.mockReturnValueOnce([]);
    const onPlaceAsset = vi.fn();
    const onConnectAsset = vi.fn();
    const { result } = renderHook(() =>
      useInternalAssetDrop({ onPlaceAsset, onConnectAsset }),
    );

    const dt = makeDataTransfer([DRAG_MIME_ASSET_ID], {
      [DRAG_MIME_ASSET_ID]: "asset-1",
    });
    act(() => result.current.onDrop(dragEvent(dt, { x: 120, y: 80 })));

    expect(onPlaceAsset).toHaveBeenCalledWith("asset-1", { x: 120, y: 80 });
    expect(onConnectAsset).not.toHaveBeenCalled();
    expect(result.current.isOver).toBe(false);
  });

  it("calls onConnectAsset with the source node id when the drop lands on a storyboard node", () => {
    getIntersectingNodes.mockReturnValueOnce([
      {
        id: "node-shot-9",
        type: "storyboard",
        data: { kind: "storyboard", shotId: "shot-9" },
      },
    ]);
    const onPlaceAsset = vi.fn();
    const onConnectAsset = vi.fn();
    const { result } = renderHook(() =>
      useInternalAssetDrop({ onPlaceAsset, onConnectAsset }),
    );

    const dt = makeDataTransfer([DRAG_MIME_ASSET_ID], {
      [DRAG_MIME_ASSET_ID]: "asset-2",
    });
    act(() => result.current.onDrop(dragEvent(dt, { x: 10, y: 10 })));

    expect(onConnectAsset).toHaveBeenCalledWith("asset-2", "node-shot-9");
    expect(onPlaceAsset).not.toHaveBeenCalled();
  });

  it("connects to a non-storyboard node as well (any node kind is upstream)", () => {
    getIntersectingNodes.mockReturnValueOnce([
      {
        id: "node-char-1",
        type: "character",
        data: { kind: "character", characterId: "c1" },
      },
    ]);
    const onPlaceAsset = vi.fn();
    const onConnectAsset = vi.fn();
    const { result } = renderHook(() =>
      useInternalAssetDrop({ onPlaceAsset, onConnectAsset }),
    );

    const dt = makeDataTransfer([DRAG_MIME_ASSET_ID], {
      [DRAG_MIME_ASSET_ID]: "asset-3",
    });
    act(() => result.current.onDrop(dragEvent(dt, { x: 50, y: 50 })));

    expect(onConnectAsset).toHaveBeenCalledWith("asset-3", "node-char-1");
    expect(onPlaceAsset).not.toHaveBeenCalled();
  });

  it("does nothing when the drop has no asset id payload", () => {
    const onPlaceAsset = vi.fn();
    const onConnectAsset = vi.fn();
    const { result } = renderHook(() =>
      useInternalAssetDrop({ onPlaceAsset, onConnectAsset }),
    );

    const dt = makeDataTransfer([DRAG_MIME_ASSET_ID]);
    act(() => result.current.onDrop(dragEvent(dt)));

    expect(onPlaceAsset).not.toHaveBeenCalled();
    expect(onConnectAsset).not.toHaveBeenCalled();
  });

  it("clears isOver on dragleave only when the pointer leaves the container", () => {
    const { result } = renderHook(() =>
      useInternalAssetDrop({ onPlaceAsset: vi.fn(), onConnectAsset: vi.fn() }),
    );

    const dt = makeDataTransfer([DRAG_MIME_ASSET_ID]);
    act(() => result.current.onDragOver(dragEvent(dt)));
    expect(result.current.isOver).toBe(true);

    // Leave from a child element — should NOT clear.
    const container = {} as EventTarget;
    const child = {} as EventTarget;
    const innerLeave = {
      currentTarget: container,
      target: child,
    } as unknown as React.DragEvent;
    act(() => result.current.onDragLeave(innerLeave));
    expect(result.current.isOver).toBe(true);

    // Leave from the container itself — should clear.
    const outerLeave = {
      currentTarget: container,
      target: container,
    } as unknown as React.DragEvent;
    act(() => result.current.onDragLeave(outerLeave));
    expect(result.current.isOver).toBe(false);
  });
});
