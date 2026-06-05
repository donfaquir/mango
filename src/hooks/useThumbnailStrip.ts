import { useQuery } from "@tanstack/react-query";
import { commands } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export function useThumbnailStrip(
  videoPath: string | null,
  intervalMs: number = 1000,
  thumbWidth: number = 120,
) {
  return useQuery({
    queryKey: ["thumbnailStrip", videoPath, intervalMs, thumbWidth],
    queryFn: () =>
      unwrap(
        commands.extractThumbnailStrip(videoPath!, intervalMs, thumbWidth),
      ),
    enabled: !!videoPath,
    staleTime: Infinity,
  });
}
