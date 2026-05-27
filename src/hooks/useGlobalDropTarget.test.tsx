import { renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

type DropEvent = { payload: { type: "drop"; paths: string[] } | { type: "enter" | "over" | "leave" } };
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

function fireDrop(paths: string[]): void {
  registeredCallback?.({ payload: { type: "drop", paths } });
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
    expect(second).toHaveBeenCalledWith(["/tmp/a.png"]);
    expect(first).not.toHaveBeenCalled();
  });

  it("restores the previous handler when the top one unmounts", () => {
    const first = vi.fn();
    const second = vi.fn();
    renderHook(() => useGlobalDropTarget(first));
    const top = renderHook(() => useGlobalDropTarget(second));

    top.unmount();
    fireDrop(["/tmp/b.png"]);
    expect(first).toHaveBeenCalledWith(["/tmp/b.png"]);
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
    registeredCallback?.({ payload: { type: "enter" } });
    registeredCallback?.({ payload: { type: "over" } });
    registeredCallback?.({ payload: { type: "leave" } });
    expect(handler).not.toHaveBeenCalled();
  });
});
