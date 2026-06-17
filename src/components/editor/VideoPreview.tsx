import { useCallback, useEffect, useRef } from "react";
import { Play, Pause } from "lucide-react";
import { cn } from "@/lib/utils";
import { useMultiTrackStore } from "@/stores/multiTrackStore";
import { usePlayheadClip } from "@/hooks/usePlayheadClip";
import { usePlayheadAudioClips, type PlayheadAudioClipInfo } from "@/hooks/usePlayheadAudioClips";
import { usePlayheadTextItems } from "@/hooks/usePlayheadTextItems";
import { useAsset } from "@/hooks/useAssets";
import { useResolvedAssetUrl } from "@/hooks/useResolvedAssetUrl";

interface VideoPreviewProps {
  projectRoot: string | undefined;
  className?: string;
}

function AudioElement({ clip, projectRoot, isPlaying }: {
  clip: PlayheadAudioClipInfo;
  projectRoot: string | undefined;
  isPlaying: boolean;
}) {
  const { data: asset } = useAsset(clip.assetId);
  const audioUrl = useResolvedAssetUrl(projectRoot, asset?.file_path ?? null);
  const audioRef = useRef<HTMLAudioElement>(null);
  const prevAssetRef = useRef<string | null>(null);

  useEffect(() => {
    const el = audioRef.current;
    if (!el || !audioUrl) return;

    if (clip.assetId !== prevAssetRef.current) {
      el.src = audioUrl;
      el.currentTime = clip.seekMs / 1000;
      prevAssetRef.current = clip.assetId;
      if (isPlaying) el.play();
      return;
    }

    if (!isPlaying) {
      el.currentTime = clip.seekMs / 1000;
    } else {
      const drift = Math.abs(el.currentTime * 1000 - clip.seekMs);
      if (drift > 300) el.currentTime = clip.seekMs / 1000;
    }
  }, [audioUrl, clip.assetId, clip.seekMs, isPlaying]);

  useEffect(() => {
    const el = audioRef.current;
    if (!el) return;
    if (isPlaying && audioUrl) {
      el.play();
    } else {
      el.pause();
    }
  }, [isPlaying, audioUrl]);

  return <audio ref={audioRef} className="hidden" />;
}

const TEXT_STYLES: Record<string, string> = {
  subtitle: "absolute bottom-4 left-0 right-0 text-center text-white text-lg font-medium [text-shadow:_1px_1px_2px_#000,_-1px_-1px_2px_#000]",
  bubble: "absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 bg-white/90 text-black rounded-xl px-4 py-2 text-sm font-medium",
  fancy: "absolute top-1/3 left-0 right-0 text-center text-amber-400 text-2xl font-bold [text-shadow:_2px_2px_4px_#000]",
  onomatopoeia: "absolute top-1/3 left-0 right-0 text-center text-red-500 text-3xl font-black italic [text-shadow:_2px_2px_4px_#000]",
};

export function VideoPreview({ projectRoot, className }: VideoPreviewProps) {
  const clipInfo = usePlayheadClip(projectRoot);
  const audioClips = usePlayheadAudioClips();
  const textItems = usePlayheadTextItems();
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
          {textItems.map((t, i) => (
            <div key={i} className={TEXT_STYLES[t.textType] ?? TEXT_STYLES.subtitle}>
              {t.content}
            </div>
          ))}
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
      {audioClips.map((clip) => (
        <AudioElement
          key={clip.assetId}
          clip={clip}
          projectRoot={projectRoot}
          isPlaying={isPlaying}
        />
      ))}
    </div>
  );
}
