import { useCallback, useEffect, useRef } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  commands,
  type Asset,
  type AssetSource,
  type AssetType,
  type ImportAssetInput,
} from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export interface UseAssetListOptions {
  projectId: string;
  assetType?: AssetType;
  source?: AssetSource;
  keyword?: string;
  limit?: number;
  offset?: number;
}

export const assetKeys = {
  all: (projectId: string, type: AssetType | "all") =>
    ["assets", projectId, type] as const,
  filtered: (
    projectId: string,
    type: AssetType | "all",
    source?: AssetSource,
    keyword?: string,
    label?: string,
  ) =>
    [
      "assets",
      projectId,
      type,
      source ?? null,
      keyword ?? null,
      label ?? null,
    ] as const,
  detail: (id: string) => ["asset", id] as const,
  labels: (projectId: string) => ["asset-labels", projectId] as const,
};

export function useAssetList(
  projectId: string | undefined,
  type?: AssetType,
  opts?: {
    source?: AssetSource;
    keyword?: string;
    /**
     * Exact-match label filter. `undefined` = any; `""` = unlabeled only;
     * non-empty string = rows whose label equals it exactly.
     */
    label?: string;
    limit?: number;
    offset?: number;
  },
) {
  const key = type ?? "all";
  return useQuery<Asset[]>({
    queryKey: projectId
      ? assetKeys.filtered(projectId, key, opts?.source, opts?.keyword, opts?.label)
      : ["assets", "none"],
    queryFn: () =>
      unwrap(
        commands.listAssets({
          project_id: projectId as string,
          asset_type: type ?? null,
          source: opts?.source ?? null,
          keyword: opts?.keyword ?? null,
          label: opts?.label ?? null,
          limit: opts?.limit ?? null,
          offset: opts?.offset ?? null,
        }),
      ),
    enabled: !!projectId,
  });
}

export function useAssetLabels(projectId: string | undefined) {
  return useQuery<string[]>({
    queryKey: projectId ? assetKeys.labels(projectId) : ["asset-labels", "none"],
    queryFn: () => unwrap(commands.listAssetLabels(projectId as string)),
    enabled: !!projectId,
  });
}

export function useAsset(id: string | null | undefined) {
  return useQuery<Asset>({
    queryKey: id ? assetKeys.detail(id) : ["asset", "none"],
    queryFn: () => unwrap(commands.getAsset(id as string)),
    enabled: !!id,
  });
}

export function useUpdateAssetLabel() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, label }: { id: string; label: string }) =>
      unwrap(commands.updateAssetLabel(id, label)),
    onSuccess: (asset) => {
      qc.setQueryData(assetKeys.detail(asset.id), asset);
      qc.invalidateQueries({ queryKey: ["assets", asset.project_id] });
      qc.invalidateQueries({ queryKey: assetKeys.labels(asset.project_id) });
    },
  });
}

export function useUpdateAssetOriginalName() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, originalName }: { id: string; originalName: string }) =>
      unwrap(commands.updateAssetOriginalName(id, originalName)),
    onSuccess: (asset) => {
      qc.setQueryData(assetKeys.detail(asset.id), asset);
      // The renamed asset shifts which `original_name` matches the keyword
      // search, so refresh the asset listings; labels are unaffected.
      qc.invalidateQueries({ queryKey: ["assets", asset.project_id] });
    },
  });
}

export function useImportAsset() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: ImportAssetInput) =>
      unwrap(commands.importAsset(input)),
    onSuccess: (_data, input) => {
      qc.invalidateQueries({ queryKey: ["assets", input.project_id] });
    },
  });
}

/**
 * 5-second grace window where a freshly "deleted" asset is hidden from list
 * caches but still alive in SQLite — long enough for the user to hit "撤销"
 * in the toast before we issue the real `commands.deleteAsset` call.
 */
const DELETE_GRACE_MS = 5000;

/**
 * Schedule an asset deletion with an optimistic-remove + undo flow.
 *
 * Returns a single function that, when called, hides the asset from every
 * `["assets", projectId, ...]` list cache, shows an undoable toast, and after
 * `DELETE_GRACE_MS` actually invokes `deleteAsset`. Undo cancels the timer
 * and re-fetches the lists so the row reappears.
 *
 * Pending deletes are tracked in a ref so the same component can have several
 * in flight; on unmount, any still-pending timers fire immediately so we
 * don't leave the DB inconsistent with what the user already saw vanish.
 */
export function useScheduleAssetDeletion(projectId: string) {
  const qc = useQueryClient();
  const timersRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());

  const invalidateLists = useCallback(() => {
    qc.invalidateQueries({ queryKey: ["assets", projectId] });
    qc.invalidateQueries({ queryKey: assetKeys.labels(projectId) });
  }, [qc, projectId]);

  const commit = useCallback(
    async (assetId: string) => {
      timersRef.current.delete(assetId);
      try {
        await unwrap(commands.deleteAsset(assetId));
      } catch (err) {
        toast.error(
          `删除失败：${err instanceof Error ? err.message : String(err)}`,
        );
      }
      invalidateLists();
    },
    [invalidateLists],
  );

  const undo = useCallback(
    (asset: Asset) => {
      const timer = timersRef.current.get(asset.id);
      if (timer !== undefined) {
        clearTimeout(timer);
        timersRef.current.delete(asset.id);
      }
      // The asset row is still in SQLite — refetching the lists brings it
      // back into every cached query under this project.
      invalidateLists();
      toast.success(`已恢复「${asset.original_name}」`);
    },
    [invalidateLists],
  );

  // Flush any pending deletes when the consumer unmounts so we don't end up
  // with a row the user already saw disappear but never actually got deleted.
  useEffect(() => {
    const timers = timersRef.current;
    return () => {
      for (const [assetId, timer] of timers.entries()) {
        clearTimeout(timer);
        void commit(assetId);
      }
      timers.clear();
    };
  }, [commit]);

  return useCallback(
    (asset: Asset) => {
      // Optimistically strip the row from every list query under this project.
      qc.setQueriesData<Asset[]>(
        { queryKey: ["assets", projectId] },
        (prev) => prev?.filter((a) => a.id !== asset.id),
      );

      const timer = setTimeout(() => {
        void commit(asset.id);
      }, DELETE_GRACE_MS);
      timersRef.current.set(asset.id, timer);

      toast.success(`已删除「${asset.original_name}」`, {
        duration: DELETE_GRACE_MS,
        action: {
          label: "撤销",
          onClick: () => undo(asset),
        },
      });
    },
    [qc, projectId, commit, undo],
  );
}

// `useRegisterProjectAssetScope` removed — the workspace-wide asset-protocol
// scope is granted once at app startup, so per-project mount registration
// is no longer needed.
