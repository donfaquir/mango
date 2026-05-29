import { useEffect } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { PhysicalPosition } from "@tauri-apps/api/dpi";

export type DropHandler = (
  paths: string[],
  position: PhysicalPosition,
) => void;

const handlerStack: DropHandler[] = [];
let listenerInstalled = false;
let unlistenPromise: Promise<() => void> | null = null;

function installListener(): void {
  if (listenerInstalled) return;
  listenerInstalled = true;
  unlistenPromise = getCurrentWebview().onDragDropEvent((event) => {
    if (event.payload.type !== "drop") return;
    // Tauri 2 fires `drop` for in-webview HTML5 drags too (with empty paths)
    // when `dragDropEnabled` is on. Treat empty paths as "not a file drop"
    // and skip dispatch so consumers don't have to filter it themselves.
    if (event.payload.paths.length === 0) return;
    const top = handlerStack[handlerStack.length - 1];
    top?.(event.payload.paths, event.payload.position);
  });
}

/**
 * Mount a global drop handler. Stack semantics: the last mounted handler
 * receives the drop event; when it unmounts, the previously active one is
 * restored. Required because `onDragDropEvent` is webview-global and would
 * fire every listener if multiple Dropzones were nested.
 *
 * The `handler` argument must be a stable reference (wrap in `useCallback`
 * at the call site), otherwise each render will push a new entry and the
 * stack will grow unbounded.
 */
export function useGlobalDropTarget(
  handler: DropHandler,
  enabled: boolean = true,
): void {
  useEffect(() => {
    if (!enabled) return;
    installListener();
    handlerStack.push(handler);
    return () => {
      const idx = handlerStack.lastIndexOf(handler);
      if (idx !== -1) handlerStack.splice(idx, 1);
    };
  }, [handler, enabled]);
}

export function _resetGlobalDropTargetForTests(): void {
  handlerStack.length = 0;
  listenerInstalled = false;
  unlistenPromise = null;
}

export function _getUnlistenPromiseForTests(): Promise<() => void> | null {
  return unlistenPromise;
}
