import { useCallback, useRef } from "react";
import { useTimelineStore } from "@/stores/timelineStore";
import { TimeScale } from "./TimeScale";
import { ThumbnailStrip } from "./ThumbnailStrip";
import { TrimHandles } from "./TrimHandles";
import { PlaybackHead } from "./PlaybackHead";
import { cn } from "@/lib/utils";
import { msToPixel, pixelToMs } from "./timelineUtils";

interface TimelineProps {
  videoPath: string | null;
  onSeek: (ms: number) => void;
  className?: string;
}

export function Timeline({ videoPath, onSeek, className }: TimelineProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const { duration, trimStart, trimEnd, playbackPosition, zoom, setTrim, setZoom } =
    useTimelineStore();

  const totalWidth = msToPixel(duration, zoom);

  const handleTimelineClick = useCallback(
    (e: React.MouseEvent) => {
      const rect = scrollRef.current?.getBoundingClientRect();
      if (!rect) return;
      const scrollLeft = scrollRef.current?.scrollLeft ?? 0;
      const clickX = e.clientX - rect.left + scrollLeft;
      const ms = Math.max(0, Math.min(pixelToMs(clickX, zoom), duration));
      onSeek(ms);
    },
    [zoom, duration, onSeek],
  );

  const handleWheel = useCallback(
    (e: React.WheelEvent) => {
      if (!e.ctrlKey && !e.metaKey) return;
      e.preventDefault();
      const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
      setZoom(zoom * factor);
    },
    [zoom, setZoom],
  );

  return (
    <div
      ref={scrollRef}
      className={cn("overflow-x-auto border border-border rounded bg-muted/30", className)}
      onWheel={handleWheel}
    >
      <div className="relative" style={{ width: totalWidth, minHeight: 88 }}>
        <TimeScale duration={duration} zoom={zoom} />
        <div className="relative" onClick={handleTimelineClick}>
          <ThumbnailStrip
            videoPath={videoPath}
            duration={duration}
            zoom={zoom}
          />
          <TrimHandles
            trimStart={trimStart}
            trimEnd={trimEnd}
            duration={duration}
            zoom={zoom}
            onTrimChange={setTrim}
          />
          <PlaybackHead positionMs={playbackPosition} zoom={zoom} />
        </div>
      </div>
    </div>
  );
}
