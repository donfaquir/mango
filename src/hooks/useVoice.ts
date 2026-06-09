import { useMutation, useQueryClient } from "@tanstack/react-query";
import { commands } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";
import { taskKeys } from "./useTasks";

export function useGenerateShotVoice() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ shotId, accountId }: { shotId: string; accountId: string }) =>
      unwrap(commands.generateShotVoice(shotId, accountId)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: taskKeys.all() });
    },
  });
}

export function useGenerateEpisodeVoices() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      episodeId,
      accountId,
    }: {
      episodeId: string;
      accountId: string;
    }) => unwrap(commands.generateEpisodeVoices(episodeId, accountId)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: taskKeys.all() });
    },
  });
}

export function usePreviewVoice() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      voiceId,
      text,
      accountId,
    }: {
      voiceId: string;
      text: string;
      accountId: string;
    }) => unwrap(commands.previewVoice(voiceId, text, accountId)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: taskKeys.all() });
    },
  });
}
