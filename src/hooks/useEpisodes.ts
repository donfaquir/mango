import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type CreateEpisodeInput,
  type Episode,
  type UpdateEpisodeInput_Deserialize,
} from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export const episodeKeys = {
  all: (projectId: string) => ["episodes", projectId] as const,
  detail: (id: string) => ["episode", id] as const,
};

export function useEpisodeList(projectId: string | undefined) {
  return useQuery<Episode[]>({
    queryKey: projectId ? episodeKeys.all(projectId) : ["episodes", "none"],
    queryFn: () =>
      unwrap(commands.listEpisodes({ project_id: projectId as string })),
    enabled: !!projectId,
  });
}

export function useEpisode(id: string | undefined) {
  return useQuery<Episode>({
    queryKey: id ? episodeKeys.detail(id) : ["episode", "none"],
    queryFn: () => unwrap(commands.getEpisode(id as string)),
    enabled: !!id,
  });
}

export function useCreateEpisode() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateEpisodeInput) =>
      unwrap(commands.createEpisode(input)),
    onSuccess: (_data, input) => {
      qc.invalidateQueries({ queryKey: episodeKeys.all(input.project_id) });
    },
  });
}

export function useUpdateEpisode(projectId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      id,
      input,
    }: {
      id: string;
      input: UpdateEpisodeInput_Deserialize;
    }) => unwrap(commands.updateEpisode(id, input)),
    onSuccess: (_data, { id }) => {
      qc.invalidateQueries({ queryKey: episodeKeys.all(projectId) });
      qc.invalidateQueries({ queryKey: episodeKeys.detail(id) });
    },
  });
}

export function useDeleteEpisode(projectId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.deleteEpisode(id)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: episodeKeys.all(projectId) });
    },
  });
}

export function useReorderEpisodes(projectId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (orderedIds: string[]) =>
      unwrap(commands.reorderEpisodes(projectId, orderedIds)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: episodeKeys.all(projectId) });
    },
  });
}
