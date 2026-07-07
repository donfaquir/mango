import { useCallback, useEffect, useRef } from "react";
import { Play, Pause } from "lucide-react";
import { cn } from "@/lib/utils";
import { useMultiTrackStore } from "@/stores/multiTrackStore";
import { usePlayheadClip } from "@/hooks/usePlayheadClip";
import { usePlayheadAudioClips, type PlayheadAudioClipInfo } from "@/hooks/usePlayheadAudioClips";
import { usePlayheadTextItems, type PlayheadTextInfo } from "@/hooks/usePlayheadTextItems";
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

function textOverlayStyle(t: PlayheadTextInfo): React.CSSProperties {
  const s = t.style;
  const outline = s?.outline_color;
  const shadow = outline
    ? `1px 1px 0 ${outline}, -1px -1px 0 ${outline}, 1px -1px 0 ${outline}, -1px 1px 0 ${outline}`
    : undefined;
  const isBubble = t.textType === "bubble";
  return {
    position: "absolute",
    left: s?.position_x != null ? `${s.position_x * 100}%` : (s?.alignment === "left" ? "0" : s?.alignment === "right" ? undefined : "0"),
    right: s?.alignment === "right" ? "0" : (s?.position_x != null ? undefined : "0"),
    top: s?.position_y != null ? `${s.position_y * 100}%` : undefined,
    bottom: s?.position_y == null && t.textType === "subtitle" ? "1rem" : undefined,
    transform: s?.position_x != null ? "translateX(-50%)" : undefined,
    fontSize: s?.font_size ? `${Math.round(s.font_size * 0.4)}px` : undefined,
    color: s?.color ?? "white",
    fontWeight: s?.font_weight === "bold" ? "bold" : "normal",
    textShadow: shadow,
    textAlign: (s?.alignment as "left" | "center" | "right") ?? "center",
    ...(isBubble ? {
      backgroundColor: t.bubble?.fill_color ?? "rgba(255,255,255,0.9)",
      borderRadius: "0.75rem",
      padding: "0.5rem 1rem",
      border: t.bubble?.border_color ? `2px solid ${t.bubble.border_color}` : undefined,
    } : {}),
  };
}

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
            <div key={i} style={textOverlayStyle(t)}>
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
