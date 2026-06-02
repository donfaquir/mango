import { useEffect, useRef } from "react";
import { useDebouncedCallback } from "use-debounce";
import { useCanvasStore } from "@/stores/canvasStore";
import { useCanvasLayoutMutation } from "@/hooks/useCanvasLayout";
import { hashLayout, serializeLayout } from "./serialization";

const AUTO_SAVE_DEBOUNCE_MS = 1000;
const IDLE_NODE_THRESHOLD = 30;

type IdleCallback = (cb: () => void) => void;

function scheduleSerialize(nodeCount: number, cb: () => void) {
  if (
    nodeCount >= IDLE_NODE_THRESHOLD &&
    typeof window !== "undefined" &&
    "requestIdleCallback" in window
  ) {
    (window as unknown as { requestIdleCallback: IdleCallback })
      .requestIdleCallback(cb);
  } else {
    cb();
  }
}

export interface AutoSaveLayoutHandle {
  /** Synchronously fire any pending debounced save. No-op if nothing is queued. */
  flush: () => void;
}

export function useAutoSaveLayout(episodeId: string): AutoSaveLayoutHandle {
  const lastHashRef = useRef<string | null>(null);
  const mutation = useCanvasLayoutMutation();
  const mutate = mutation.mutate;

  const save = useDebouncedCallback(() => {
    const { nodes, edges, viewport } = useCanvasStore.getState();
    scheduleSerialize(nodes.length, () => {
      let payload;
      try {
        payload = serializeLayout({ nodes, edges, viewport });
      } catch (err) {
        console.error("[canvas] failed to serialize layout, skipping save", err);
        return;
      }
      const hash = hashLayout(payload);
      if (hash === lastHashRef.current) return;
      lastHashRef.current = hash;
      mutate({ episode_id: episodeId, ...payload });
    });
  }, AUTO_SAVE_DEBOUNCE_MS);

  useEffect(() => {
    const unsubscribe = useCanvasStore.subscribe(() => save());
    return () => {
      unsubscribe();
      // Flush any pending debounced save so route changes don't drop the last
      // edit. use-debounce's flush is a no-op if nothing is pending.
      save.flush();
    };
  }, [save]);

  // Seed the hash on first run from whatever is currently in the store so a
  // load-only render doesn't re-write the same layout back to disk.
  useEffect(() => {
    const { nodes, edges, viewport } = useCanvasStore.getState();
    lastHashRef.current = hashLayout(
      serializeLayout({ nodes, edges, viewport }),
    );
  }, [episodeId]);

  return { flush: () => save.flush() };
}
