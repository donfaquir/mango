import { useCallback, useRef } from "react";
import { cn } from "@/lib/utils";
import { msToPixel, pixelToMs } from "./timelineUtils";

interface TrimHandlesProps {
  trimStart: number;
  trimEnd: number;
  duration: number;
  zoom: number;
  onTrimChange: (start: number, end: number) => void;
  className?: string;
}

const MIN_DURATION_MS = 100;
const SNAP_THRESHOLD_MS = 50;

function snapToSecond(ms: number, shiftKey: boolean): number {
  if (shiftKey) return ms;
  const nearest = Math.round(ms / 1000) * 1000;
  return Math.abs(ms - nearest) <= SNAP_THRESHOLD_MS ? nearest : ms;
}

export function TrimHandles({
  trimStart,
  trimEnd,
  duration,
  zoom,
  onTrimChange,
  className,
}: TrimHandlesProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const totalWidth = msToPixel(duration, zoom);
  const leftPx = msToPixel(trimStart, zoom);
  const rightPx = msToPixel(trimEnd, zoom);

  const handleDrag = useCallback(
    (side: "left" | "right", e: React.PointerEvent) => {
      e.preventDefault();
      const startX = e.clientX;
      const startMs = side === "left" ? trimStart : trimEnd;

      const onMove = (ev: PointerEvent) => {
        const delta = ev.clientX - startX;
        const deltaMs = pixelToMs(delta, zoom);
        let newMs = snapToSecond(startMs + deltaMs, ev.shiftKey);

        if (side === "left") {
          newMs = Math.max(0, Math.min(newMs, trimEnd - MIN_DURATION_MS));
          onTrimChange(newMs, trimEnd);
        } else {
          newMs = Math.max(trimStart + MIN_DURATION_MS, Math.min(newMs, duration));
          onTrimChange(trimStart, newMs);
        }
      };

      const onUp = () => {
        document.removeEventListener("pointermove", onMove);
        document.removeEventListener("pointerup", onUp);
      };

      document.addEventListener("pointermove", onMove);
      document.addEventListener("pointerup", onUp);
    },
    [trimStart, trimEnd, duration, zoom, onTrimChange],
  );

  return (
    <div
      ref={containerRef}
      className={cn("absolute inset-0 pointer-events-none", className)}
      style={{ width: totalWidth }}
    >
      {/* Left excluded region */}
      <div
        className="absolute inset-y-0 left-0 bg-black/50"
        style={{ width: leftPx }}
      />

      {/* Right excluded region */}
      <div
        className="absolute inset-y-0 bg-black/50"
        style={{ left: rightPx, width: totalWidth - rightPx }}
      />

      {/* Selected region border */}
      <div
        className="absolute inset-y-0 border-y-2 border-primary/70"
        style={{ left: leftPx, width: rightPx - leftPx }}
      />

      {/* Left handle */}
      <div
        className="absolute inset-y-0 w-3 cursor-col-resize pointer-events-auto
                   bg-primary/80 hover:bg-primary flex items-center justify-center"
        style={{ left: leftPx - 6 }}
        onPointerDown={(e) => handleDrag("left", e)}
      >
        <div className="space-y-0.5">
          <div className="h-px w-1.5 bg-primary-foreground/70" />
          <div className="h-px w-1.5 bg-primary-foreground/70" />
          <div className="h-px w-1.5 bg-primary-foreground/70" />
        </div>
      </div>

      {/* Right handle */}
      <div
        className="absolute inset-y-0 w-3 cursor-col-resize pointer-events-auto
                   bg-primary/80 hover:bg-primary flex items-center justify-center"
        style={{ left: rightPx - 6 }}
        onPointerDown={(e) => handleDrag("right", e)}
      >
        <div className="space-y-0.5">
          <div className="h-px w-1.5 bg-primary-foreground/70" />
          <div className="h-px w-1.5 bg-primary-foreground/70" />
          <div className="h-px w-1.5 bg-primary-foreground/70" />
        </div>
      </div>
    </div>
  );
}
