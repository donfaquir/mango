import { forwardRef, useCallback, useEffect, useImperativeHandle, useRef } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useTimelineStore } from "@/stores/timelineStore";
import { cn } from "@/lib/utils";

interface VideoPreviewProps {
  videoPath: string | null;
  className?: string;
}

export interface VideoPreviewHandle {
  seekTo: (ms: number) => void;
}

export const VideoPreview = forwardRef<VideoPreviewHandle, VideoPreviewProps>(
  function VideoPreview({ videoPath, className }, ref) {
    const videoRef = useRef<HTMLVideoElement>(null);
    const rafRef = useRef<number>(0);
    const { isPlaying, setPlaybackPosition, togglePlay } = useTimelineStore();

    useImperativeHandle(ref, () => ({
      seekTo: (ms: number) => {
        if (videoRef.current) {
          videoRef.current.currentTime = ms / 1000;
          setPlaybackPosition(ms);
        }
      },
    }));

    const syncPlayback = useCallback(() => {
      if (videoRef.current) {
        setPlaybackPosition(videoRef.current.currentTime * 1000);
      }
      rafRef.current = requestAnimationFrame(syncPlayback);
    }, [setPlaybackPosition]);

    useEffect(() => {
      if (isPlaying) {
        videoRef.current?.play();
        rafRef.current = requestAnimationFrame(syncPlayback);
      } else {
        videoRef.current?.pause();
        cancelAnimationFrame(rafRef.current);
      }
      return () => cancelAnimationFrame(rafRef.current);
    }, [isPlaying, syncPlayback]);

    const handleToggle = useCallback(() => {
      togglePlay();
    }, [togglePlay]);

    if (!videoPath) {
      return (
        <div className={cn("flex items-center justify-center bg-muted text-muted-foreground h-64", className)}>
          No video selected
        </div>
      );
    }

    return (
      <div className={cn("relative bg-black", className)}>
        <video
          ref={videoRef}
          src={convertFileSrc(videoPath)}
          className="w-full max-h-80 object-contain"
          onClick={handleToggle}
        />
        <button
          onClick={handleToggle}
          className="absolute bottom-3 left-3 px-3 py-1 rounded bg-black/60 text-white text-xs hover:bg-black/80"
        >
          {isPlaying ? "Pause" : "Play"}
        </button>
      </div>
    );
  },
);
