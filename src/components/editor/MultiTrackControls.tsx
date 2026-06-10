import { Slider } from "@/components/ui/slider";
import { useMultiTrackStore } from "@/stores/multiTrackStore";

function formatTime(ms: number): string {
  const min = Math.floor(ms / 60_000);
  const sec = Math.floor((ms % 60_000) / 1000);
  const frac = Math.floor((ms % 1000) / 10);
  return `${min}:${sec.toString().padStart(2, "0")}.${frac.toString().padStart(2, "0")}`;
}

export function MultiTrackControls() {
  const { playhead, zoom, setZoom, totalDuration } = useMultiTrackStore();

  return (
    <div className="flex items-center gap-4 px-3 py-1.5 border-t border-border bg-muted/20 text-xs">
      <span className="font-mono text-muted-foreground w-24">
        {formatTime(playhead)} / {formatTime(totalDuration)}
      </span>
      <div className="flex items-center gap-2 ml-auto">
        <span className="text-muted-foreground">Zoom</span>
        <Slider
          className="w-28"
          min={0.1}
          max={20}
          step={0.1}
          value={[zoom]}
          onValueChange={([v]) => setZoom(v)}
        />
        <span className="font-mono w-10 text-right">{zoom.toFixed(1)}x</span>
      </div>
    </div>
  );
}
