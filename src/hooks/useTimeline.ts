import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { commands } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export const timelineKeys = {
  tracks: (episodeId: string) => ["timelineTracks", episodeId] as const,
};

export function useTimelineTracks(episodeId: string | undefined) {
  return useQuery({
    queryKey: timelineKeys.tracks(episodeId ?? ""),
    queryFn: () => unwrap(commands.listTimelineTracks(episodeId!)),
    enabled: !!episodeId,
  });
}

export function useImportAudio(episodeId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => unwrap(commands.importAudioFromShots(episodeId)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: timelineKeys.tracks(episodeId) });
    },
  });
}
