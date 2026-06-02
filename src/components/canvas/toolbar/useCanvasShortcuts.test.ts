import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook } from "@testing-library/react";

const undo = vi.fn();
const redo = vi.fn();

vi.mock("@/stores/canvasStore", () => ({
  useCanvasStore: {
    temporal: {
      getState: () => ({ undo, redo }),
    },
  },
}));

import { useCanvasShortcuts } from "./useCanvasShortcuts";

const platformDescriptor = Object.getOwnPropertyDescriptor(
  window.navigator,
  "platform",
);

function setPlatform(value: string) {
  Object.defineProperty(window.navigator, "platform", {
    value,
    configurable: true,
  });
}

function dispatchKey(
  key: string,
  init: KeyboardEventInit & { target?: EventTarget } = {},
) {
  const event = new KeyboardEvent("keydown", {
    key,
    bubbles: true,
    cancelable: true,
    ...init,
  });
  if (init.target) {
    Object.defineProperty(event, "target", { value: init.target });
  }
  document.dispatchEvent(event);
  return event;
}

describe("useCanvasShortcuts", () => {
  beforeEach(() => {
    undo.mockReset();
    redo.mockReset();
    setPlatform("MacIntel");
  });

  afterEach(() => {
    if (platformDescriptor) {
      Object.defineProperty(window.navigator, "platform", platformDescriptor);
    }
  });

  it("Cmd+Z triggers undo on macOS", () => {
    renderHook(() => useCanvasShortcuts());
    const event = dispatchKey("z", { metaKey: true });
    expect(undo).toHaveBeenCalledTimes(1);
    expect(redo).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(true);
  });

  it("Cmd+Shift+Z triggers redo on macOS", () => {
    renderHook(() => useCanvasShortcuts());
    const event = dispatchKey("z", { metaKey: true, shiftKey: true });
    expect(redo).toHaveBeenCalledTimes(1);
    expect(undo).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(true);
  });

  it("Cmd+Y is intentionally NOT redo on macOS", () => {
    renderHook(() => useCanvasShortcuts());
    dispatchKey("y", { metaKey: true });
    expect(redo).not.toHaveBeenCalled();
    expect(undo).not.toHaveBeenCalled();
  });

  it("Ctrl+Y triggers redo on non-macOS platforms", () => {
    setPlatform("Win32");
    renderHook(() => useCanvasShortcuts());
    dispatchKey("y", { ctrlKey: true });
    expect(redo).toHaveBeenCalledTimes(1);
  });

  it("Ctrl+Z triggers undo on non-macOS platforms", () => {
    setPlatform("Win32");
    renderHook(() => useCanvasShortcuts());
    dispatchKey("z", { ctrlKey: true });
    expect(undo).toHaveBeenCalledTimes(1);
  });

  it("ignores keys without the platform modifier", () => {
    renderHook(() => useCanvasShortcuts());
    dispatchKey("z", {});
    expect(undo).not.toHaveBeenCalled();
    expect(redo).not.toHaveBeenCalled();
  });

  it("skips when focus is in an INPUT", () => {
    renderHook(() => useCanvasShortcuts());
    const input = document.createElement("input");
    document.body.appendChild(input);
    input.focus();
    dispatchKey("z", { metaKey: true, target: input });
    expect(undo).not.toHaveBeenCalled();
    input.remove();
  });

  it("skips when focus is in a contentEditable element", () => {
    renderHook(() => useCanvasShortcuts());
    const div = document.createElement("div");
    div.contentEditable = "true";
    document.body.appendChild(div);
    dispatchKey("z", { metaKey: true, target: div });
    expect(undo).not.toHaveBeenCalled();
    div.remove();
  });

  it("removes the listener on unmount", () => {
    const { unmount } = renderHook(() => useCanvasShortcuts());
    unmount();
    dispatchKey("z", { metaKey: true });
    expect(undo).not.toHaveBeenCalled();
  });
});
