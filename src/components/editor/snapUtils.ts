import type { TimelineItem } from "@/lib/bindings/commands";

export interface SnapTarget {
  snapMs: number;
  type: "playhead" | "item-start" | "item-end" | "second";
}

export function findSnapTarget(
  positionMs: number,
  allItems: TimelineItem[],
  excludeId: string,
  playhead: number,
  thresholdMs: number,
): SnapTarget | null {
  let best: SnapTarget | null = null;
  let bestDist = thresholdMs;

  const check = (ms: number, type: SnapTarget["type"]) => {
    const dist = Math.abs(positionMs - ms);
    if (dist < bestDist) {
      bestDist = dist;
      best = { snapMs: ms, type };
    }
  };

  check(playhead, "playhead");

  for (const item of allItems) {
    if (item.id === excludeId) continue;
    check(item.position_ms, "item-start");
    check(item.position_ms + item.duration_ms, "item-end");
  }

  const nearestSecond = Math.round(positionMs / 1000) * 1000;
  check(nearestSecond, "second");

  return best;
}
