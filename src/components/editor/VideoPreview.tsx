import { useCallback, useEffect, useRef } from "react";
import { Play, Pause } from "lucide-react";
import { cn } from "@/lib/utils";
import { useMultiTrackStore } from "@/stores/multiTrackStore";
import { usePlayheadClip } from "@/hooks/usePlayheadClip";

interface VideoPreviewProps {
  projectRoot: string | undefined;
  className?: string;
}

export function VideoPreview({ projectRoot, className }: VideoPreviewProps) {
  const clipInfo = usePlayheadClip(projectRoot);
  const isPlaying = useMultiTrackStore((s) => s.isPlaying);
  const togglePlay = useMultiTrackStore((s) => s.togglePlay);
  const videoRef = useRef<HTMLVideoElement>(null);
  const prevAssetRef = useRef<string | null>(null);

  useEffect(() => {
    const video = videoRef.current;
    if (!video || !clipInfo?.videoUrl) return;

    if (clipInfo.assetId !== prevAssetRef.current) {
      video.src = clipInfo.videoUrl;
      video.currentTime = clipInfo.seekMs / 1000;
      prevAssetRef.current = clipInfo.assetId;
      if (isPlaying) video.play();
      return;
    }

    if (!isPlaying) {
      video.currentTime = clipInfo.seekMs / 1000;
    } else {
      const drift = Math.abs(video.currentTime * 1000 - clipInfo.seekMs);
      if (drift > 300) {
        video.currentTime = clipInfo.seekMs / 1000;
      }
    }
  }, [clipInfo?.videoUrl, clipInfo?.assetId, clipInfo?.seekMs, isPlaying]);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    if (isPlaying && clipInfo?.videoUrl) {
      video.play();
    } else {
      video.pause();
    }
  }, [isPlaying, clipInfo?.videoUrl]);

  useEffect(() => {
    if (!clipInfo) {
      prevAssetRef.current = null;
    }
  }, [clipInfo]);

  const handleClick = useCallback(() => {
    togglePlay();
  }, [togglePlay]);

  return (
    <div
      className={cn(
        "relative bg-black flex items-center justify-center",
        className,
      )}
    >
      {clipInfo?.videoUrl ? (
        <>
          <video
            ref={videoRef}
            className="max-h-full max-w-full object-contain"
            onClick={handleClick}
          />
          <button
            onClick={handleClick}
            className="absolute bottom-2 left-2 p-1.5 rounded bg-black/60 text-white hover:bg-black/80"
          >
            {isPlaying ? (
              <Pause className="size-4" />
            ) : (
              <Play className="size-4" />
            )}
          </button>
        </>
      ) : (
        <span className="text-sm text-muted-foreground">
          播放头处无片段
        </span>
      )}
    </div>
  );
}
