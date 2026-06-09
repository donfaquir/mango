import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type CreateShotAudioInput,
  type ShotAudio,
  type UpdateShotAudioInput_Deserialize,
} from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export const shotAudioKeys = {
  all: (shotId: string) => ["shot-audio", shotId] as const,
};

export function useShotAudioList(shotId: string | undefined) {
  return useQuery<ShotAudio[]>({
    queryKey: shotId ? shotAudioKeys.all(shotId) : ["shot-audio", "none"],
    queryFn: () => unwrap(commands.listShotAudio(shotId as string)),
    enabled: !!shotId,
  });
}

export function useCreateShotAudio(shotId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateShotAudioInput) =>
      unwrap(commands.createShotAudio(input)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: shotAudioKeys.all(shotId) });
    },
  });
}

export function useUpdateShotAudio(shotId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      id,
      input,
    }: {
      id: string;
      input: UpdateShotAudioInput_Deserialize;
    }) => unwrap(commands.updateShotAudio(id, input)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: shotAudioKeys.all(shotId) });
    },
  });
}

export function useDeleteShotAudio(shotId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.deleteShotAudio(id)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: shotAudioKeys.all(shotId) });
    },
  });
}
