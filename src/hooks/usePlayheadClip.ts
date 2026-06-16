import { useMemo } from "react";
import { useMultiTrackStore } from "@/stores/multiTrackStore";
import { useAsset } from "@/hooks/useAssets";
import { useResolvedAssetUrl } from "@/hooks/useResolvedAssetUrl";

export interface PlayheadClipInfo {
  assetId: string;
  videoUrl: string | null;
  seekMs: number;
}

export function usePlayheadClip(
  projectRoot: string | undefined,
): PlayheadClipInfo | null {
  const tracks = useMultiTrackStore((s) => s.tracks);
  const items = useMultiTrackStore((s) => s.items);
  const playhead = useMultiTrackStore((s) => s.playhead);

  const activeClip = useMemo(() => {
    const videoTrack = tracks.find(
      (t) => t.track_type === "video" && !t.muted,
    );
    if (!videoTrack) return null;

    const trackItems = Object.values(items).filter(
      (i) =>
        i.track_id === videoTrack.id &&
        i.item_type === "clip" &&
        !!i.asset_id,
    );

    for (const item of trackItems) {
      const start = item.position_ms;
      const end = start + item.duration_ms;
      if (playhead >= start && playhead < end) {
        const seekMs = playhead - start + item.in_point_ms;
        return { assetId: item.asset_id!, seekMs };
      }
    }
    return null;
  }, [tracks, items, playhead]);

  const { data: asset } = useAsset(activeClip?.assetId);
  const videoUrl = useResolvedAssetUrl(
    projectRoot,
    asset?.file_path ?? null,
  );

  if (!activeClip) return null;

  return {
    assetId: activeClip.assetId,
    videoUrl,
    seekMs: activeClip.seekMs,
  };
}
