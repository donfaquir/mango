import { useMemo } from "react";
import { useMultiTrackStore } from "@/stores/multiTrackStore";

export interface PlayheadTextStyle {
  font_size?: number; font_weight?: string; color?: string;
  outline_color?: string; outline_width?: number; shadow?: boolean;
  position_x?: number; position_y?: number; alignment?: string;
}

export interface PlayheadBubbleStyle {
  shape?: string; fill_color?: string; border_color?: string;
}

export interface PlayheadTextInfo {
  content: string;
  textType: string;
  style?: PlayheadTextStyle;
  bubble?: PlayheadBubbleStyle;
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
              style: typeof params.style === "object" && params.style !== null
                ? params.style as PlayheadTextStyle : undefined,
              bubble: typeof params.bubble === "object" && params.bubble !== null
                ? params.bubble as PlayheadBubbleStyle : undefined,
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
