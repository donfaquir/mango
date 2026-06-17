import { useCallback, useRef } from "react";
import { Plus, Film, Music, Type, Layers } from "lucide-react";
import { toast } from "sonner";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { msToPixel, pixelToMs } from "./timelineUtils";
import { useMultiTrackStore } from "@/stores/multiTrackStore";
import { TimeScale } from "./TimeScale";
import { PlaybackHead } from "./PlaybackHead";
import { TrackHeader } from "./TrackHeader";
import { TimelineItemBlock } from "./TimelineItemBlock";
import type { TrackType } from "@/lib/bindings/commands";

const TRACK_HEIGHT = 64;
const HEADER_WIDTH = 160;

const ADD_TRACK_OPTIONS: { type: TrackType; label: string; icon: React.ElementType }[] = [
  { type: "video", label: "视频", icon: Film },
  { type: "audio", label: "音频", icon: Music },
  { type: "text", label: "文本", icon: Type },
  { type: "overlay", label: "叠加层", icon: Layers },
];

interface MultiTrackTimelineProps {
  episodeId: string;
  projectRoot: string | undefined;
}

export function MultiTrackTimeline({ episodeId, projectRoot }: MultiTrackTimelineProps) {
  const {
    tracks, items, playhead, zoom, totalDuration, setPlayhead, setZoom,
    addTrack, removeTrack, toggleMuted, toggleLocked,
  } = useMultiTrackStore();
  const scrollRef = useRef<HTMLDivElement>(null);
  const duration = Math.max(totalDuration, 10_000);
  const timelineWidth = msToPixel(duration, zoom);

  const handleTimelineClick = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      const rect = e.currentTarget.getBoundingClientRect();
      const x = e.clientX - rect.left + (scrollRef.current?.scrollLeft ?? 0);
      const ms = pixelToMs(x, zoom);
      setPlayhead(Math.max(0, Math.min(ms, duration)));
    },
    [zoom, duration, setPlayhead],
  );

  const handleWheel = useCallback(
    (e: React.WheelEvent) => {
      if (e.ctrlKey || e.metaKey) {
        e.preventDefault();
        const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
        setZoom(zoom * factor);
      }
    },
    [zoom, setZoom],
  );

  const handleToggleMute = useCallback(
    (trackId: string) => { toggleMuted(trackId); },
    [toggleMuted],
  );

  const handleToggleLock = useCallback(
    (trackId: string) => { toggleLocked(trackId); },
    [toggleLocked],
  );

  const handleDeleteTrack = useCallback(async (trackId: string, label: string) => {
    if (!window.confirm(`Delete track "${label}"? All items on this track will be removed.`)) return;
    try {
      await removeTrack(trackId);
    } catch (err) {
      toast.error(`删除轨道失败：${err instanceof Error ? err.message : String(err)}`);
    }
  }, [removeTrack]);

  const handleAddTrack = useCallback(async (trackType: TrackType) => {
    try {
      await addTrack({ episode_id: episodeId, track_type: trackType, label: trackType });
    } catch (err) {
      toast.error(`添加轨道失败：${err instanceof Error ? err.message : String(err)}`);
    }
  }, [episodeId, addTrack]);

  const itemsByTrack = new Map<string, typeof items[string][]>();
  for (const item of Object.values(items)) {
    const arr = itemsByTrack.get(item.track_id) ?? [];
    arr.push(item);
    itemsByTrack.set(item.track_id, arr);
  }

  return (
    <div className="flex border border-border rounded-sm overflow-hidden bg-background">
      {/* Track headers (fixed left) */}
      <div className="shrink-0 border-r border-border" style={{ width: HEADER_WIDTH }}>
        <div className="h-6 border-b border-border" />
        {tracks.map((track) => (
          <TrackHeader
            key={track.id}
            track={track}
            onToggleMute={() => handleToggleMute(track.id)}
            onToggleLock={() => handleToggleLock(track.id)}
            onDelete={() => handleDeleteTrack(track.id, track.label)}
          />
        ))}
        <div className="flex items-center justify-center py-1">
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="sm" className="h-6 text-xs gap-1 text-muted-foreground">
                <Plus className="size-3" />
                添加轨道
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent>
              {ADD_TRACK_OPTIONS.map(({ type, label, icon: Icon }) => (
                <DropdownMenuItem key={type} onClick={() => handleAddTrack(type)}>
                  <Icon className="size-4" />
                  {label}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>

      {/* Scrollable timeline area */}
      <div
        ref={scrollRef}
        className="flex-1 overflow-x-auto overflow-y-hidden"
        onWheel={handleWheel}
      >
        <div className="relative" style={{ width: timelineWidth }}>
          <TimeScale duration={duration} zoom={zoom} />

          <div onClick={handleTimelineClick}>
            {tracks.map((track, trackIdx) => (
              <div
                key={track.id}
                className={cn(
                  "relative border-b border-border",
                  track.muted && "opacity-40",
                )}
                style={{ height: TRACK_HEIGHT }}
              >
                {(itemsByTrack.get(track.id) ?? []).map((item) => (
                  <TimelineItemBlock
                    key={item.id}
                    item={item}
                    zoom={zoom}
                    trackLocked={track.locked}
                    trackType={track.track_type}
                    projectRoot={projectRoot}
                    tracks={tracks}
                    trackIndex={trackIdx}
                  />
                ))}
              </div>
            ))}
          </div>

          <PlaybackHead positionMs={playhead} zoom={zoom} />
        </div>
      </div>
    </div>
  );
}
