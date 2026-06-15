import { useQuery } from "@tanstack/react-query";
import { commands } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export function useWaveform(
  audioPath: string | null,
  height: number = 64,
  pxPerSec: number = 50,
) {
  return useQuery({
    queryKey: ["waveform", audioPath, height, pxPerSec],
    queryFn: () =>
      unwrap(commands.extractWaveform(audioPath!, height, pxPerSec)),
    enabled: !!audioPath,
    staleTime: Infinity,
  });
}
