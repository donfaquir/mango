import { Trash2, GripVertical } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { VideoClip } from "@/lib/bindings/commands";
import { cn } from "@/lib/utils";

interface ClipCardProps {
  clip: VideoClip;
  onDelete: (id: string) => void;
  className?: string;
  dragHandleProps?: Record<string, unknown>;
}

function formatMs(ms: number): string {
  const s = Math.floor(ms / 1000);
  const m = Math.floor(s / 60);
  const sec = s % 60;
  const tenths = Math.floor((ms % 1000) / 100);
  return `${m}:${sec.toString().padStart(2, "0")}.${tenths}`;
}

function formatRange(startMs: number | null, endMs: number | null): string {
  if (startMs == null && endMs == null) return "完整片段";
  return `${formatMs(startMs ?? 0)} – ${endMs != null ? formatMs(endMs) : "结尾"}`;
}

function formatDuration(startMs: number | null, endMs: number | null): string {
  if (startMs == null || endMs == null) return "";
  const dur = (endMs - startMs) / 1000;
  return `${dur.toFixed(1)}s`;
}

export function ClipCard({ clip, onDelete, className, dragHandleProps }: ClipCardProps) {
  return (
    <div
      className={cn(
        "flex items-center gap-2 px-2 py-1.5 bg-card border border-border rounded text-sm",
        className,
      )}
    >
      <div className="cursor-grab text-muted-foreground" {...dragHandleProps}>
        <GripVertical className="h-4 w-4" />
      </div>
      <div className="flex-1 min-w-0">
        <div className="truncate font-medium text-xs">
          {clip.label || clip.source_asset_id.slice(0, 8)}
        </div>
        <div className="text-[10px] text-muted-foreground">
          {formatRange(clip.trim_start_ms, clip.trim_end_ms)}
          {" "}
          {formatDuration(clip.trim_start_ms, clip.trim_end_ms)}
        </div>
      </div>
      <Button
        variant="ghost"
        size="icon"
        className="h-6 w-6"
        onClick={() => onDelete(clip.id)}
      >
        <Trash2 className="h-3 w-3" />
      </Button>
    </div>
  );
}
