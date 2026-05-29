import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

type Position = { x: number; y: number };
type DropEvent = {
  payload:
    | { type: "drop"; paths: string[]; position: Position }
    | { type: "enter"; paths: string[]; position: Position }
    | { type: "over"; position: Position }
    | { type: "leave" };
};
type DropCallback = (event: DropEvent) => void;

const onDragDropEvent = vi.fn<(cb: DropCallback) => Promise<() => void>>();
let registeredCallback: DropCallback | null = null;

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: (cb: DropCallback) => onDragDropEvent(cb),
  }),
}));

import {
  useGlobalDropTarget,
  _resetGlobalDropTargetForTests,
} from "./useGlobalDropTarget";

const ORIGIN: Position = { x: 0, y: 0 };

function fireDrop(paths: string[], position: Position = ORIGIN): void {
  registeredCallback?.({ payload: { type: "drop", paths, position } });
}

beforeEach(() => {
  _resetGlobalDropTargetForTests();
  onDragDropEvent.mockReset();
  registeredCallback = null;
  onDragDropEvent.mockImplementation(async (cb) => {
    registeredCallback = cb;
    return () => {};
  });
});

describe("useGlobalDropTarget", () => {
  it("installs the webview listener exactly once across multiple mounts", () => {
    const a = vi.fn();
    const b = vi.fn();
    renderHook(() => useGlobalDropTarget(a));
    renderHook(() => useGlobalDropTarget(b));
    expect(onDragDropEvent).toHaveBeenCalledTimes(1);
  });

  it("dispatches drops to the most recently mounted handler", () => {
    const first = vi.fn();
    const second = vi.fn();
    renderHook(() => useGlobalDropTarget(first));
    renderHook(() => useGlobalDropTarget(second));

    fireDrop(["/tmp/a.png"]);
    expect(second).toHaveBeenCalledWith(["/tmp/a.png"], ORIGIN);
    expect(first).not.toHaveBeenCalled();
  });

  it("forwards the drop position alongside the paths", () => {
    const handler = vi.fn();
    renderHook(() => useGlobalDropTarget(handler));
    const pos = { x: 100, y: 50 };
    fireDrop(["/tmp/d.png"], pos);
    expect(handler).toHaveBeenCalledWith(["/tmp/d.png"], pos);
  });

  it("restores the previous handler when the top one unmounts", () => {
    const first = vi.fn();
    const second = vi.fn();
    renderHook(() => useGlobalDropTarget(first));
    const top = renderHook(() => useGlobalDropTarget(second));

    top.unmount();
    fireDrop(["/tmp/b.png"]);
    expect(first).toHaveBeenCalledWith(["/tmp/b.png"], ORIGIN);
    expect(second).not.toHaveBeenCalled();
  });

  it("does not subscribe when enabled is false", () => {
    const handler = vi.fn();
    renderHook(() => useGlobalDropTarget(handler, false));
    fireDrop(["/tmp/c.png"]);
    expect(handler).not.toHaveBeenCalled();
  });

  it("ignores non-drop drag events", () => {
    const handler = vi.fn();
    renderHook(() => useGlobalDropTarget(handler));
    registeredCallback?.({ payload: { type: "enter", paths: [], position: ORIGIN } });
    registeredCallback?.({ payload: { type: "over", position: ORIGIN } });
    registeredCallback?.({ payload: { type: "leave" } });
    expect(handler).not.toHaveBeenCalled();
  });

  it("ignores drop events with empty paths (in-webview HTML5 drags)", () => {
    const handler = vi.fn();
    renderHook(() => useGlobalDropTarget(handler));
    fireDrop([]);
    expect(handler).not.toHaveBeenCalled();
  });
});
