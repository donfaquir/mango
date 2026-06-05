import { cn } from "@/lib/utils";
import { msToPixel } from "./timelineUtils";

interface PlaybackHeadProps {
  positionMs: number;
  zoom: number;
  className?: string;
}

export function PlaybackHead({ positionMs, zoom, className }: PlaybackHeadProps) {
  const left = msToPixel(positionMs, zoom);

  return (
    <div
      className={cn("absolute top-0 bottom-0 pointer-events-none z-10", className)}
      style={{ left }}
    >
      {/* Triangle marker */}
      <div className="w-0 h-0 -translate-x-1 border-l-[5px] border-r-[5px] border-t-[6px] border-l-transparent border-r-transparent border-t-red-500" />
      {/* Vertical line */}
      <div className="w-0.5 h-full bg-red-500 -translate-x-px" />
    </div>
  );
}
