import { convertFileSrc } from "@tauri-apps/api/core";
import { Skeleton } from "@/components/ui/skeleton";
import { useThumbnailStrip } from "@/hooks/useThumbnailStrip";
import { cn } from "@/lib/utils";
import { msToPixel } from "./timelineUtils";

interface ThumbnailStripProps {
  videoPath: string | null;
  duration: number;
  zoom: number;
  className?: string;
}

export function ThumbnailStrip({
  videoPath,
  duration,
  zoom,
  className,
}: ThumbnailStripProps) {
  const { data, isLoading } = useThumbnailStrip(videoPath);
  const totalWidth = msToPixel(duration, zoom);

  if (isLoading || !data) {
    return (
      <Skeleton
        className={cn("h-16 rounded-none", className)}
        style={{ width: totalWidth }}
      />
    );
  }

  const thumbCount = data.thumbnails.length;
  const thumbDisplayWidth = thumbCount > 0 ? totalWidth / thumbCount : 0;

  return (
    <div
      className={cn("flex h-16 overflow-hidden", className)}
      style={{ width: totalWidth }}
    >
      {data.thumbnails.map((path, i) => (
        <img
          key={i}
          src={convertFileSrc(path)}
          alt=""
          className="h-full object-cover"
          style={{ width: thumbDisplayWidth }}
          draggable={false}
        />
      ))}
    </div>
  );
}
