import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type CreateShotInput,
  type Shot,
  type ShotLinks,
  type SubjectKind,
  type UpdateShotInput_Deserialize,
} from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export const shotKeys = {
  all: (episodeId: string) => ["shots", episodeId] as const,
  detail: (id: string) => ["shot", id] as const,
  links: (id: string) => ["shot", id, "links"] as const,
};

export function useShotList(episodeId: string | undefined) {
  return useQuery<Shot[]>({
    queryKey: episodeId ? shotKeys.all(episodeId) : ["shots", "none"],
    queryFn: () =>
      unwrap(commands.listShots({ episode_id: episodeId as string })),
    enabled: !!episodeId,
  });
}

export function useShot(id: string | undefined) {
  return useQuery<Shot>({
    queryKey: id ? shotKeys.detail(id) : ["shot", "none"],
    queryFn: () => unwrap(commands.getShot(id as string)),
    enabled: !!id,
  });
}

export function useShotLinks(id: string | undefined) {
  return useQuery<ShotLinks>({
    queryKey: id ? shotKeys.links(id) : ["shot", "none", "links"],
    queryFn: () => unwrap(commands.listShotLinks(id as string)),
    enabled: !!id,
  });
}

export function useCreateShot() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateShotInput) => unwrap(commands.createShot(input)),
    onSuccess: (_data, input) => {
      qc.invalidateQueries({ queryKey: shotKeys.all(input.episode_id) });
    },
  });
}

export function useUpdateShot(episodeId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      id,
      input,
    }: {
      id: string;
      input: UpdateShotInput_Deserialize;
    }) => unwrap(commands.updateShot(id, input)),
    onSuccess: (_data, { id }) => {
      qc.invalidateQueries({ queryKey: shotKeys.all(episodeId) });
      qc.invalidateQueries({ queryKey: shotKeys.detail(id) });
    },
  });
}

export function useDeleteShot(episodeId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.deleteShot(id)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: shotKeys.all(episodeId) });
    },
  });
}

export function useReorderShots(episodeId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (orderedIds: string[]) =>
      unwrap(commands.reorderShots(episodeId, orderedIds)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: shotKeys.all(episodeId) });
    },
  });
}

export function useLinkShotSubject() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      shotId,
      subjectId,
      subjectKind,
    }: {
      shotId: string;
      subjectId: string;
      subjectKind: SubjectKind;
    }) => unwrap(commands.linkShotSubject(shotId, subjectId, subjectKind)),
    onSuccess: (_data, { shotId }) => {
      qc.invalidateQueries({ queryKey: shotKeys.links(shotId) });
    },
  });
}

export function useAdoptTaskResult(episodeId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ shotId, taskId }: { shotId: string; taskId: string }) =>
      unwrap(commands.adoptTaskResult(shotId, taskId)),
    onSuccess: (shot) => {
      qc.invalidateQueries({ queryKey: shotKeys.all(episodeId) });
      qc.invalidateQueries({ queryKey: shotKeys.detail(shot.id) });
    },
  });
}

export function useUnadoptShot(episodeId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (shotId: string) => unwrap(commands.unadoptShot(shotId)),
    onSuccess: (shot) => {
      qc.invalidateQueries({ queryKey: shotKeys.all(episodeId) });
      qc.invalidateQueries({ queryKey: shotKeys.detail(shot.id) });
    },
  });
}

export function useUnlinkShotSubject() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      shotId,
      subjectId,
      subjectKind,
    }: {
      shotId: string;
      subjectId: string;
      subjectKind: SubjectKind;
    }) => unwrap(commands.unlinkShotSubject(shotId, subjectId, subjectKind)),
    onSuccess: (_data, { shotId }) => {
      qc.invalidateQueries({ queryKey: shotKeys.links(shotId) });
    },
  });
}
