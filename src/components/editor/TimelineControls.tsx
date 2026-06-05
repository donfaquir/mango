import { Slider } from "@/components/ui/slider";
import { cn } from "@/lib/utils";

interface TimelineControlsProps {
  zoom: number;
  onZoomChange: (zoom: number) => void;
  playbackPosition: number;
  trimStart: number;
  trimEnd: number;
  className?: string;
}

const MIN_ZOOM = 0.1;
const MAX_ZOOM = 20;

function formatTime(ms: number): string {
  const totalSec = Math.floor(ms / 1000);
  const min = Math.floor(totalSec / 60);
  const sec = totalSec % 60;
  const millis = Math.floor(ms % 1000);
  return `${min}:${sec.toString().padStart(2, "0")}.${millis.toString().padStart(3, "0")}`;
}

function zoomToSlider(zoom: number): number {
  return (Math.log(zoom / MIN_ZOOM) / Math.log(MAX_ZOOM / MIN_ZOOM)) * 100;
}

function sliderToZoom(value: number): number {
  return MIN_ZOOM * Math.pow(MAX_ZOOM / MIN_ZOOM, value / 100);
}

export function TimelineControls({
  zoom,
  onZoomChange,
  playbackPosition,
  trimStart,
  trimEnd,
  className,
}: TimelineControlsProps) {
  return (
    <div className={cn("flex items-center gap-4 px-3 py-1.5 border-t border-border text-xs text-muted-foreground", className)}>
      <span className="font-mono">{formatTime(playbackPosition)}</span>
      <span className="text-muted-foreground/50">|</span>
      <span>
        {formatTime(trimStart)} – {formatTime(trimEnd)}
      </span>
      <div className="ml-auto flex items-center gap-2">
        <span className="text-[10px]">{zoom.toFixed(1)}x</span>
        <Slider
          className="w-24"
          min={0}
          max={100}
          step={1}
          value={[zoomToSlider(zoom)]}
          onValueChange={([v]) => onZoomChange(sliderToZoom(v))}
        />
      </div>
    </div>
  );
}
