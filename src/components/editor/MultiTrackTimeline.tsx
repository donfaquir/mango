import { useCallback, useRef } from "react";
import { cn } from "@/lib/utils";
import { msToPixel, pixelToMs } from "./timelineUtils";
import { useMultiTrackStore } from "@/stores/multiTrackStore";
import { TimeScale } from "./TimeScale";
import { PlaybackHead } from "./PlaybackHead";
import { TrackHeader } from "./TrackHeader";
import { TimelineItemBlock } from "./TimelineItemBlock";
import { commands } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

const TRACK_HEIGHT = 64;
const HEADER_WIDTH = 160;

export function MultiTrackTimeline() {
  const { tracks, items, playhead, zoom, totalDuration, setPlayhead, setZoom } =
    useMultiTrackStore();
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

  const handleToggleMute = useCallback(async (trackId: string, muted: boolean) => {
    await unwrap(commands.reorderTimelineTracks([]));
    useMultiTrackStore.setState((s) => ({
      tracks: s.tracks.map((t) => (t.id === trackId ? { ...t, muted: !muted } : t)),
    }));
  }, []);

  const handleToggleLock = useCallback(async (trackId: string, locked: boolean) => {
    useMultiTrackStore.setState((s) => ({
      tracks: s.tracks.map((t) => (t.id === trackId ? { ...t, locked: !locked } : t)),
    }));
  }, []);

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
            onToggleMute={() => handleToggleMute(track.id, track.muted)}
            onToggleLock={() => handleToggleLock(track.id, track.locked)}
          />
        ))}
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
            {tracks.map((track) => (
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
