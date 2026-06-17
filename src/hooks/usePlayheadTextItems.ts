import { useMemo } from "react";
import { useMultiTrackStore } from "@/stores/multiTrackStore";

export interface PlayheadTextInfo {
  content: string;
  textType: string;
}

export function usePlayheadTextItems(): PlayheadTextInfo[] {
  const tracks = useMultiTrackStore((s) => s.tracks);
  const items = useMultiTrackStore((s) => s.items);
  const playhead = useMultiTrackStore((s) => s.playhead);

  return useMemo(() => {
    const textTracks = tracks.filter(
      (t) => t.track_type === "text" && !t.muted,
    );

    const result: PlayheadTextInfo[] = [];
    for (const track of textTracks) {
      const trackItems = Object.values(items).filter(
        (i) => i.track_id === track.id && i.item_type === "text",
      );

      for (const item of trackItems) {
        const start = item.position_ms;
        const end = start + item.duration_ms;
        if (playhead >= start && playhead < end) {
          try {
            const params = JSON.parse(item.params_json) as Record<string, unknown>;
            result.push({
              content: typeof params.content === "string" ? params.content : "",
              textType: typeof params.text_type === "string" ? params.text_type : "subtitle",
            });
          } catch {
            // skip malformed params
          }
        }
      }
    }
    return result;
  }, [tracks, items, playhead]);
}
