import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  events,
  type CreateCheckpointInput,
  type EpisodeCheckpointListItem,
} from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";
import { canvasLayoutKey } from "./useCanvasLayout";
import { shotKeys } from "./useShots";

export const checkpointKeys = {
  all: (episodeId: string) => ["checkpoints", episodeId] as const,
};

export function useCreateCheckpoint() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateCheckpointInput) =>
      unwrap(commands.createEpisodeCheckpoint(input)),
    onSuccess: (_data, input) => {
      qc.invalidateQueries({
        queryKey: checkpointKeys.all(input.episode_id),
      });
    },
  });
}

export function useCheckpointList(episodeId: string | undefined) {
  return useQuery<EpisodeCheckpointListItem[]>({
    queryKey: episodeId
      ? checkpointKeys.all(episodeId)
      : ["checkpoints", "none"],
    queryFn: () =>
      unwrap(commands.listEpisodeCheckpoints(episodeId as string)),
    enabled: !!episodeId,
  });
}

export function useRestoreCheckpoint(episodeId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (checkpointId: string) =>
      unwrap(commands.restoreEpisodeCheckpoint(checkpointId)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: shotKeys.all(episodeId) });
      qc.invalidateQueries({ queryKey: checkpointKeys.all(episodeId) });
      qc.invalidateQueries({ queryKey: ["episode", episodeId] });
      qc.invalidateQueries({ queryKey: canvasLayoutKey(episodeId) });
    },
  });
}

export function useDeleteCheckpoint(episodeId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) =>
      unwrap(commands.deleteEpisodeCheckpoint(id)),
    onSuccess: () => {
      qc.invalidateQueries({
        queryKey: checkpointKeys.all(episodeId),
      });
    },
  });
}

export function useEpisodeDataRestoredListener() {
  const qc = useQueryClient();
  useEffect(() => {
    const unlisten = events.episodeDataRestored.listen((event) => {
      const eid = event.payload.episode_id;
      qc.invalidateQueries({ queryKey: shotKeys.all(eid) });
      qc.invalidateQueries({ queryKey: checkpointKeys.all(eid) });
      qc.invalidateQueries({ queryKey: ["episode", eid] });
      qc.invalidateQueries({ queryKey: canvasLayoutKey(eid) });
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [qc]);
}
