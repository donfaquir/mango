import { useEffect } from "react";
import { toast } from "sonner";
import { useTimelineStore } from "@/stores/timelineStore";
import { Timeline } from "./Timeline";
import { TimelineControls } from "./TimelineControls";
import { cn } from "@/lib/utils";
import { commands } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

interface VideoEditorLayoutProps {
  videoPath: string;
  className?: string;
}

/** @deprecated Legacy single-clip layout. Use MultiTrackEditor instead. */
export function VideoEditorLayout({ videoPath, className }: VideoEditorLayoutProps) {
  const {
    zoom,
    setZoom,
    playbackPosition,
    trimStart,
    trimEnd,
    setVideoSrc,
    reset,
  } = useTimelineStore();

  useEffect(() => {
    let cancelled = false;
    unwrap(commands.probeVideo(videoPath))
      .then((meta) => {
        if (!cancelled) {
          setVideoSrc(videoPath, meta.duration_ms);
        }
      })
      .catch((e) => {
        if (!cancelled) {
          toast.error(`视频探测失败：${e instanceof Error ? e.message : String(e)}`);
        }
      });
    return () => {
      cancelled = true;
      reset();
    };
  }, [videoPath, setVideoSrc, reset]);

  return (
    <div className={cn("flex flex-col gap-0", className)}>
      <Timeline videoPath={videoPath} onSeek={() => {}} />
      <TimelineControls
        zoom={zoom}
        onZoomChange={setZoom}
        playbackPosition={playbackPosition}
        trimStart={trimStart}
        trimEnd={trimEnd}
      />
    </div>
  );
}
