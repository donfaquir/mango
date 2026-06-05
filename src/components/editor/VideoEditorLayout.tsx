import { useCallback, useEffect, useRef } from "react";
import { toast } from "sonner";
import { useTimelineStore } from "@/stores/timelineStore";
import { VideoPreview, type VideoPreviewHandle } from "./VideoPreview";
import { Timeline } from "./Timeline";
import { TimelineControls } from "./TimelineControls";
import { cn } from "@/lib/utils";
import { commands } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

interface VideoEditorLayoutProps {
  videoPath: string;
  videoSrcUrl: string | null;
  className?: string;
}

export function VideoEditorLayout({ videoPath, videoSrcUrl, className }: VideoEditorLayoutProps) {
  const previewRef = useRef<VideoPreviewHandle>(null);
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

  const handleSeek = useCallback((ms: number) => {
    previewRef.current?.seekTo(ms);
  }, []);

  return (
    <div className={cn("flex flex-col gap-0", className)}>
      <VideoPreview ref={previewRef} videoSrcUrl={videoSrcUrl} />
      <Timeline videoPath={videoPath} onSeek={handleSeek} />
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
