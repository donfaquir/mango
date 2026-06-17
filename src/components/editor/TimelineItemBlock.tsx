import { useMemo } from "react";
import { cn } from "@/lib/utils";
import { msToPixel } from "./timelineUtils";
import { useMultiTrackStore } from "@/stores/multiTrackStore";
import { useItemDrag } from "@/hooks/useItemDrag";
import { VideoClipThumbnails } from "./VideoClipThumbnails";
import { AudioClipWaveform } from "./AudioClipWaveform";
import { ItemContextMenu } from "./ItemContextMenu";
import { ItemLabel } from "./ItemLabel";
import type { TimelineItem, TimelineTrack } from "@/lib/bindings/commands";

interface TimelineItemBlockProps {
  item: TimelineItem;
  zoom: number;
  trackLocked: boolean;
  trackType: string;
  projectRoot: string | undefined;
  tracks: TimelineTrack[];
  trackIndex: number;
}

const ITEM_COLORS: Record<string, string> = {
  clip: "bg-blue-500/70 border-blue-600",
  text: "bg-emerald-500/70 border-emerald-600",
  transition: "bg-orange-500/70 border-orange-600",
  sticker: "bg-purple-500/70 border-purple-600",
  effect: "bg-yellow-500/70 border-yellow-600",
};

const MEDIA_CLIP_BORDER = "border-blue-600";

export function TimelineItemBlock({
  item, zoom, trackLocked, trackType, projectRoot, tracks, trackIndex,
}: TimelineItemBlockProps) {
  const selected = useMultiTrackStore((s) => s.selection.has(item.id));
  const { handleMouseDown, handleLeftTrimDown, handleRightTrimDown, snapLineMs } =
    useItemDrag(item, zoom, trackLocked, trackType, tracks, trackIndex);

  const isVideoClip = trackType === "video" && item.item_type === "clip" && !!item.asset_id;
  const isAudioClip = trackType === "audio" && item.item_type === "clip" && !!item.asset_id;

  const parsedParams = useMemo(() => {
    try {
      return JSON.parse(item.params_json) as Record<string, unknown>;
    } catch {
      return null;
    }
  }, [item.params_json]);

  const left = msToPixel(item.position_ms, zoom);
  const width = Math.max(msToPixel(item.duration_ms, zoom), 4);

  const hasMediaViz = isVideoClip || isAudioClip;
  const colorClass = hasMediaViz
    ? `bg-slate-900 ${MEDIA_CLIP_BORDER}`
    : ITEM_COLORS[item.item_type] ?? "bg-gray-500/70 border-gray-600";

  return (
    <ItemContextMenu itemId={item.id} trackLocked={trackLocked}>
      <div
        className={cn(
          "absolute top-1 bottom-1 rounded-sm border cursor-grab active:cursor-grabbing",
          "overflow-hidden select-none group",
          colorClass,
          selected && "ring-2 ring-blue-400 ring-offset-1 ring-offset-background",
          trackLocked && "opacity-50 cursor-not-allowed",
        )}
        style={{ left, width }}
        onMouseDown={handleMouseDown}
      >
        {/* Left trim handle */}
        <div
          className="absolute left-0 top-0 bottom-0 w-1 cursor-col-resize bg-white/0 hover:bg-white/40 opacity-0 group-hover:opacity-100 z-10"
          onMouseDown={handleLeftTrimDown}
        />

        {isVideoClip && (
          <VideoClipThumbnails assetId={item.asset_id!} projectRoot={projectRoot} />
        )}
        {isAudioClip && (
          <AudioClipWaveform assetId={item.asset_id!} projectRoot={projectRoot} />
        )}
        <div className={cn(
          "flex items-center gap-1 px-1.5 h-full",
          hasMediaViz && "absolute inset-0 bg-black/30",
        )}>
          <ItemLabel item={item} trackType={trackType} params={parsedParams} />
        </div>

        {/* Right trim handle */}
        <div
          className="absolute right-0 top-0 bottom-0 w-1 cursor-col-resize bg-white/0 hover:bg-white/40 opacity-0 group-hover:opacity-100 z-10"
          onMouseDown={handleRightTrimDown}
        />

        {/* Snap line */}
        {snapLineMs !== null && (
          <div
            className="absolute top-0 bottom-0 w-px bg-yellow-400 z-20 pointer-events-none"
            style={{ left: msToPixel(snapLineMs - item.position_ms, zoom) }}
          />
        )}
      </div>
    </ItemContextMenu>
  );
}
