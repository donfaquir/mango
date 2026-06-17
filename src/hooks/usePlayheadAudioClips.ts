import { useMemo } from "react";
import { useMultiTrackStore } from "@/stores/multiTrackStore";

export interface PlayheadAudioClipInfo {
  assetId: string;
  seekMs: number;
}

export function usePlayheadAudioClips(): PlayheadAudioClipInfo[] {
  const tracks = useMultiTrackStore((s) => s.tracks);
  const items = useMultiTrackStore((s) => s.items);
  const playhead = useMultiTrackStore((s) => s.playhead);

  return useMemo(() => {
    const audioTracks = tracks.filter(
      (t) => t.track_type === "audio" && !t.muted,
    );

    const result: PlayheadAudioClipInfo[] = [];
    for (const track of audioTracks) {
      const trackItems = Object.values(items).filter(
        (i) =>
          i.track_id === track.id &&
          i.item_type === "clip" &&
          !!i.asset_id,
      );

      for (const item of trackItems) {
        const start = item.position_ms;
        const end = start + item.duration_ms;
        if (playhead >= start && playhead < end) {
          result.push({
            assetId: item.asset_id!,
            seekMs: playhead - start + item.in_point_ms,
          });
          break;
        }
      }
    }
    return result;
  }, [tracks, items, playhead]);
}
