import { useEffect } from "react";
import { useCanvasStore } from "@/stores/canvasStore";

function isMacPlatform(): boolean {
  return (
    typeof navigator !== "undefined" &&
    /Mac|iPhone|iPad/.test(navigator.platform)
  );
}

/**
 * Global Cmd+Z / Cmd+Shift+Z (Ctrl+Y) handler that drives the canvas
 * temporal store. macOS deliberately does NOT accept Cmd+Y for redo —
 * that's a system-reserved shortcut elsewhere in the macOS UX.
 *
 * Skips form controls so the browser's native undo for text inputs keeps
 * working unchanged.
 */
export function useCanvasShortcuts(): void {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      if (target) {
        const ce = target.contentEditable;
        const editable =
          target.isContentEditable || ce === "true" || ce === "plaintext-only";
        if (
          target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          editable
        ) {
          return;
        }
      }

      const isMac = isMacPlatform();
      const mod = isMac ? e.metaKey : e.ctrlKey;
      if (!mod) return;

      const key = e.key.toLowerCase();

      if (key === "z" && !e.shiftKey) {
        e.preventDefault();
        useCanvasStore.temporal.getState().undo();
        return;
      }

      const isRedo =
        (key === "z" && e.shiftKey) || (!isMac && key === "y");
      if (isRedo) {
        e.preventDefault();
        useCanvasStore.temporal.getState().redo();
      }
    };

    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, []);
}
