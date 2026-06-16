import { useEffect, useRef } from "react";
import { useMultiTrackStore } from "@/stores/multiTrackStore";

export function usePlaybackLoop(): void {
  const isPlaying = useMultiTrackStore((s) => s.isPlaying);
  const rafRef = useRef<number>(0);
  const lastTimeRef = useRef<number>(0);

  useEffect(() => {
    if (!isPlaying) {
      cancelAnimationFrame(rafRef.current);
      return;
    }

    lastTimeRef.current = performance.now();

    const tick = (now: number) => {
      const delta = now - lastTimeRef.current;
      lastTimeRef.current = now;
      const { playhead, totalDuration, pause, setPlayhead } =
        useMultiTrackStore.getState();
      const next = playhead + delta;
      if (next >= totalDuration) {
        setPlayhead(totalDuration);
        pause();
        return;
      }
      setPlayhead(next);
      rafRef.current = requestAnimationFrame(tick);
    };

    rafRef.current = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafRef.current);
  }, [isPlaying]);
}
