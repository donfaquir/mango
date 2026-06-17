import { useCallback, useRef, useState } from "react";
import { pixelToMs } from "@/components/editor/timelineUtils";
import { findSnapTarget } from "@/components/editor/snapUtils";
import { useMultiTrackStore } from "@/stores/multiTrackStore";
import type { TimelineItem, TimelineTrack } from "@/lib/bindings/commands";

const TRACK_HEIGHT = 64;
const MIN_DURATION_MS = 100;

interface DragState {
  startX: number;
  startY: number;
  startPos: number;
  mode: "move" | "trim-left" | "trim-right";
}

export function useItemDrag(
  item: TimelineItem,
  zoom: number,
  trackLocked: boolean,
  trackType: string,
  tracks: TimelineTrack[],
  trackIndex: number,
) {
  const dragRef = useRef<DragState | null>(null);
  const [snapLineMs, setSnapLineMs] = useState<number | null>(null);

  const getAllItems = () => Object.values(useMultiTrackStore.getState().items);
  const getPlayhead = () => useMultiTrackStore.getState().playhead;
  const snapThresholdMs = () => pixelToMs(8, zoom);

  const startDrag = useCallback(
    (e: React.MouseEvent, mode: DragState["mode"]) => {
      if (trackLocked) return;
      e.stopPropagation();
      if (mode === "move") {
        useMultiTrackStore.getState().selectItem(item.id, e.shiftKey);
      }
      dragRef.current = { startX: e.clientX, startY: e.clientY, startPos: item.position_ms, mode };

      const handleMouseMove = (ev: MouseEvent) => {
        const drag = dragRef.current;
        if (!drag) return;
        const dx = ev.clientX - drag.startX;
        const deltaMs = pixelToMs(dx, zoom);

        if (drag.mode === "move") {
          let newPos = Math.max(0, drag.startPos + deltaMs);
          const snap = findSnapTarget(newPos, getAllItems(), item.id, getPlayhead(), snapThresholdMs());
          if (snap) {
            newPos = snap.snapMs;
            setSnapLineMs(snap.snapMs);
          } else {
            setSnapLineMs(null);
          }

          const dy = ev.clientY - drag.startY;
          const trackOffset = Math.round(dy / TRACK_HEIGHT);
          const targetIdx = Math.max(0, Math.min(tracks.length - 1, trackIndex + trackOffset));
          const targetTrack = tracks[targetIdx];

          if (targetTrack && targetTrack.track_type === trackType) {
            useMultiTrackStore.setState((s) => ({
              items: {
                ...s.items,
                [item.id]: { ...item, position_ms: newPos, track_id: targetTrack.id },
              },
              proxyState: "stale",
            }));
          } else {
            useMultiTrackStore.setState((s) => ({
              items: { ...s.items, [item.id]: { ...item, position_ms: newPos } },
              proxyState: "stale",
            }));
          }
        } else if (drag.mode === "trim-left") {
          const newInDelta = deltaMs;
          const maxDelta = item.duration_ms - MIN_DURATION_MS;
          const clampedDelta = Math.max(-item.in_point_ms, Math.min(maxDelta, newInDelta));
          const newInPoint = item.in_point_ms + clampedDelta;
          const newPos = item.position_ms + clampedDelta;
          const newDuration = item.duration_ms - clampedDelta;

          useMultiTrackStore.setState((s) => ({
            items: {
              ...s.items,
              [item.id]: { ...item, in_point_ms: newInPoint, position_ms: newPos, duration_ms: newDuration },
            },
            proxyState: "stale",
          }));
        } else if (drag.mode === "trim-right") {
          const newDuration = Math.max(MIN_DURATION_MS, item.duration_ms + deltaMs);
          const newOutPoint = item.in_point_ms + newDuration;

          useMultiTrackStore.setState((s) => ({
            items: {
              ...s.items,
              [item.id]: { ...item, duration_ms: newDuration, out_point_ms: newOutPoint },
            },
            proxyState: "stale",
          }));
        }
      };

      const handleMouseUp = (ev: MouseEvent) => {
        document.removeEventListener("mousemove", handleMouseMove);
        document.removeEventListener("mouseup", handleMouseUp);
        setSnapLineMs(null);
        const drag = dragRef.current;
        if (!drag) return;
        const dx = ev.clientX - drag.startX;
        const dy = ev.clientY - drag.startY;
        dragRef.current = null;

        if (drag.mode === "move" && Math.abs(dx) <= 2 && Math.abs(dy) <= 2) return;

        const currentItem = useMultiTrackStore.getState().items[item.id];
        if (!currentItem) return;

        if (drag.mode === "move") {
          const trackOffset = Math.round(dy / TRACK_HEIGHT);
          const targetIdx = Math.max(0, Math.min(tracks.length - 1, trackIndex + trackOffset));
          const targetTrack = tracks[targetIdx];
          if (targetTrack && targetTrack.track_type === trackType && targetTrack.id !== item.track_id) {
            useMultiTrackStore.getState().moveItem(item.id, {
              track_id: targetTrack.id,
              position_ms: currentItem.position_ms,
            });
          } else {
            useMultiTrackStore.getState().updateItem(item.id, {
              position_ms: currentItem.position_ms,
              duration_ms: null,
              in_point_ms: null,
              out_point_ms: null,
              params_json: null,
            });
          }
        } else {
          useMultiTrackStore.getState().updateItem(item.id, {
            position_ms: currentItem.position_ms,
            duration_ms: currentItem.duration_ms,
            in_point_ms: currentItem.in_point_ms,
            out_point_ms: currentItem.out_point_ms,
            params_json: null,
          });
        }
      };

      document.addEventListener("mousemove", handleMouseMove);
      document.addEventListener("mouseup", handleMouseUp);
    },
    [item, zoom, trackLocked, trackType, tracks, trackIndex],
  );

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => startDrag(e, "move"),
    [startDrag],
  );

  const handleLeftTrimDown = useCallback(
    (e: React.MouseEvent) => startDrag(e, "trim-left"),
    [startDrag],
  );

  const handleRightTrimDown = useCallback(
    (e: React.MouseEvent) => startDrag(e, "trim-right"),
    [startDrag],
  );

  return { handleMouseDown, handleLeftTrimDown, handleRightTrimDown, snapLineMs };
}
