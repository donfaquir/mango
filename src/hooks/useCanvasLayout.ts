import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type CanvasLayout,
  type UpsertCanvasLayoutInput,
} from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export const canvasLayoutKey = (episodeId: string) =>
  ["canvas_layout", episodeId] as const;

export function useCanvasLayoutQuery(episodeId: string) {
  return useQuery<CanvasLayout | null>({
    queryKey: canvasLayoutKey(episodeId),
    queryFn: () => unwrap(commands.getCanvasLayout(episodeId)),
    staleTime: Infinity,
  });
}

export function useCanvasLayoutMutation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: UpsertCanvasLayoutInput) =>
      unwrap(commands.upsertCanvasLayout(input)),
    onSuccess: (data) => {
      qc.setQueryData(canvasLayoutKey(data.episode_id), data);
    },
  });
}
