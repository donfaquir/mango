import { useCallback, useEffect, useState } from "react";
import { events, type FfmpegProgressTick } from "@/lib/bindings/commands";

export function useFFmpegProgress(): {
  progress: FfmpegProgressTick | null;
  reset: () => void;
} {
  const [progress, setProgress] = useState<FfmpegProgressTick | null>(null);

  useEffect(() => {
    const unlisten = events.ffmpegProgressTick.listen((e) => {
      setProgress(e.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const reset = useCallback(() => setProgress(null), []);

  return { progress, reset };
}
