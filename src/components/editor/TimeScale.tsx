import { useMemo } from "react";
import { cn } from "@/lib/utils";
import { msToPixel } from "./timelineUtils";

interface TimeScaleProps {
  duration: number;
  zoom: number;
  className?: string;
}

interface TickConfig {
  intervalMs: number;
  format: (ms: number) => string;
}

function getTickConfig(zoom: number): TickConfig {
  if (zoom < 0.5) return { intervalMs: 60_000, format: formatTime };
  if (zoom < 2) return { intervalMs: 10_000, format: formatTime };
  if (zoom < 5) return { intervalMs: 5_000, format: formatTime };
  if (zoom < 10) return { intervalMs: 1_000, format: formatTime };
  return { intervalMs: 500, format: formatTimeMs };
}

function formatTime(ms: number): string {
  const min = Math.floor(ms / 60_000);
  const sec = Math.floor((ms % 60_000) / 1000);
  return `${min}:${sec.toString().padStart(2, "0")}`;
}

function formatTimeMs(ms: number): string {
  const min = Math.floor(ms / 60_000);
  const secTotal = (ms % 60_000) / 1000;
  return `${min}:${secTotal.toFixed(1).padStart(4, "0")}`;
}

export function TimeScale({ duration, zoom, className }: TimeScaleProps) {
  const { ticks, subTickCount } = useMemo(() => {
    const config = getTickConfig(zoom);
    const result: { pos: number; label: string }[] = [];
    for (let ms = 0; ms <= duration; ms += config.intervalMs) {
      result.push({ pos: msToPixel(ms, zoom), label: config.format(ms) });
    }
    return { ticks: result, subTickCount: 4 };
  }, [duration, zoom]);

  const totalWidth = msToPixel(duration, zoom);

  return (
    <div
      className={cn("relative h-6 select-none border-b border-border", className)}
      style={{ width: totalWidth }}
    >
      {ticks.map((tick, i) => (
        <div key={i}>
          <div
            className="absolute top-0 h-3 w-px bg-muted-foreground/60"
            style={{ left: tick.pos }}
          />
          <span
            className="absolute top-3 text-[10px] text-muted-foreground -translate-x-1/2"
            style={{ left: tick.pos }}
          >
            {tick.label}
          </span>
          {i < ticks.length - 1 &&
            Array.from({ length: subTickCount }).map((_, j) => {
              const subPos =
                tick.pos +
                ((ticks[i + 1].pos - tick.pos) / (subTickCount + 1)) * (j + 1);
              return (
                <div
                  key={`sub-${j}`}
                  className="absolute top-0 h-1.5 w-px bg-muted-foreground/30"
                  style={{ left: subPos }}
                />
              );
            })}
        </div>
      ))}
    </div>
  );
}
