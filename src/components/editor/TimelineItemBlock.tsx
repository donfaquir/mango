import { useCallback, useRef } from "react";
import { cn } from "@/lib/utils";
import { msToPixel, pixelToMs } from "./timelineUtils";
import { useMultiTrackStore } from "@/stores/multiTrackStore";
import { VideoClipThumbnails } from "./VideoClipThumbnails";
import type { TimelineItem } from "@/lib/bindings/commands";

interface TimelineItemBlockProps {
  item: TimelineItem;
  zoom: number;
  trackLocked: boolean;
  trackType: string;
  projectRoot: string | undefined;
}

const ITEM_COLORS: Record<string, string> = {
  clip: "bg-blue-500/70 border-blue-600",
  text: "bg-emerald-500/70 border-emerald-600",
  transition: "bg-orange-500/70 border-orange-600",
  sticker: "bg-purple-500/70 border-purple-600",
  effect: "bg-yellow-500/70 border-yellow-600",
};

export function TimelineItemBlock({ item, zoom, trackLocked, trackType, projectRoot }: TimelineItemBlockProps) {
  const { selection, selectItem, updateItem } = useMultiTrackStore();
  const selected = selection.has(item.id);
  const dragRef = useRef<{ startX: number; startPos: number } | null>(null);

  const isVideoClip = trackType === "video" && item.item_type === "clip" && !!item.asset_id;

  const left = msToPixel(item.position_ms, zoom);
  const width = Math.max(msToPixel(item.duration_ms, zoom), 4);

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if (trackLocked) return;
      e.stopPropagation();
      selectItem(item.id, e.shiftKey);

      dragRef.current = { startX: e.clientX, startPos: item.position_ms };

      const handleMouseMove = (ev: MouseEvent) => {
        if (!dragRef.current) return;
        const dx = ev.clientX - dragRef.current.startX;
        const deltaMs = pixelToMs(dx, zoom);
        const newPos = Math.max(0, dragRef.current.startPos + deltaMs);
        useMultiTrackStore.setState((s) => ({
          items: { ...s.items, [item.id]: { ...item, position_ms: newPos } },
          proxyState: "stale",
        }));
      };

      const handleMouseUp = (ev: MouseEvent) => {
        document.removeEventListener("mousemove", handleMouseMove);
        document.removeEventListener("mouseup", handleMouseUp);
        if (!dragRef.current) return;
        const dx = ev.clientX - dragRef.current.startX;
        if (Math.abs(dx) > 2) {
          const deltaMs = pixelToMs(dx, zoom);
          const newPos = Math.max(0, dragRef.current.startPos + deltaMs);
          updateItem(item.id, {
            position_ms: newPos,
            duration_ms: null,
            in_point_ms: null,
            out_point_ms: null,
            params_json: null,
          });
        }
        dragRef.current = null;
      };

      document.addEventListener("mousemove", handleMouseMove);
      document.addEventListener("mouseup", handleMouseUp);
    },
    [item, zoom, trackLocked, selectItem, updateItem],
  );

  const colorClass = ITEM_COLORS[item.item_type] ?? "bg-gray-500/70 border-gray-600";

  return (
    <div
      className={cn(
        "absolute top-1 bottom-1 rounded-sm border cursor-grab active:cursor-grabbing",
        "overflow-hidden select-none",
        colorClass,
        selected && "ring-2 ring-blue-400 ring-offset-1 ring-offset-background",
        trackLocked && "opacity-50 cursor-not-allowed",
      )}
      style={{ left, width }}
      onMouseDown={handleMouseDown}
    >
      {isVideoClip && (
        <VideoClipThumbnails
          assetId={item.asset_id!}
          projectRoot={projectRoot}
        />
      )}
      <div className={cn(
        "flex items-center px-1.5 h-full",
        isVideoClip && "absolute inset-0",
      )}>
        <span className="text-[10px] text-white font-medium truncate drop-shadow-sm">
          {item.item_type === "clip" ? "Clip" : item.item_type}
        </span>
      </div>
    </div>
  );
}
