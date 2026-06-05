import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type CreateVideoClipInput,
  type UpdateVideoClipInput,
} from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";
import { toast } from "sonner";

export const videoClipKeys = {
  list: (episodeId: string) => ["videoClips", episodeId] as const,
};

export function useVideoClipList(episodeId: string | undefined) {
  return useQuery({
    queryKey: videoClipKeys.list(episodeId ?? ""),
    queryFn: () => unwrap(commands.listVideoClips(episodeId!)),
    enabled: !!episodeId,
  });
}

export function useCreateVideoClip(episodeId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateVideoClipInput) =>
      unwrap(commands.createVideoClip(input)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: videoClipKeys.list(episodeId) });
      toast.success("片段已添加");
    },
    onError: (e) => toast.error(`添加片段失败：${e.message}`),
  });
}

export function useUpdateVideoClip(episodeId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, input }: { id: string; input: UpdateVideoClipInput }) =>
      unwrap(commands.updateVideoClip(id, input)),
    onSuccess: () => qc.invalidateQueries({ queryKey: videoClipKeys.list(episodeId) }),
    onError: (e) => toast.error(`更新片段失败：${e.message}`),
  });
}

export function useDeleteVideoClip(episodeId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.deleteVideoClip(id)),
    onSuccess: () => qc.invalidateQueries({ queryKey: videoClipKeys.list(episodeId) }),
    onError: (e) => toast.error(`删除片段失败：${e.message}`),
  });
}

export function useReorderVideoClips(episodeId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (ids: string[]) => unwrap(commands.reorderVideoClips(ids)),
    onSuccess: () => qc.invalidateQueries({ queryKey: videoClipKeys.list(episodeId) }),
    onError: (e) => toast.error(`排序失败：${e.message}`),
  });
}
