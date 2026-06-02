import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
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

// Keep the query cache in sync with the latest save. Without this, navigating
// away from the canvas and back would re-render with the cached (now stale)
// layout from the first fetch, then CanvasContainer's init() would overwrite
// the live store with that stale snapshot — and autoSave would persist the
// stale data right back to disk. CanvasContainer's init effect is guarded by
// an episodeId ref so this cache update does NOT trigger a re-init (which
// would clear the zundo undo stack).
export function useCanvasLayoutMutation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: UpsertCanvasLayoutInput) =>
      unwrap(commands.upsertCanvasLayout(input)),
    onSuccess: (data) => {
      qc.setQueryData(canvasLayoutKey(data.episode_id), data);
    },
    onError: (err) => toast.error(`画布保存失败：${String(err)}`),
  });
}
