import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
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
  filtered: (projectId: string, type: AssetType | "all", source?: AssetSource, keyword?: string) =>
    ["assets", projectId, type, source ?? null, keyword ?? null] as const,
  detail: (id: string) => ["asset", id] as const,
};

export function useAssetList(
  projectId: string | undefined,
  type?: AssetType,
  opts?: { source?: AssetSource; keyword?: string; limit?: number; offset?: number },
) {
  const key = type ?? "all";
  return useQuery<Asset[]>({
    queryKey: projectId
      ? assetKeys.filtered(projectId, key, opts?.source, opts?.keyword)
      : ["assets", "none"],
    queryFn: () =>
      unwrap(
        commands.listAssets({
          project_id: projectId as string,
          asset_type: type ?? null,
          source: opts?.source ?? null,
          keyword: opts?.keyword ?? null,
          limit: opts?.limit ?? null,
          offset: opts?.offset ?? null,
        }),
      ),
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
    mutationFn: ({ id, label }: { id: string; projectId: string; label: string }) =>
      unwrap(commands.updateAssetLabel(id, label)),
    onSuccess: (_data, { id, projectId }) => {
      qc.invalidateQueries({ queryKey: ["assets", projectId] });
      qc.invalidateQueries({ queryKey: assetKeys.detail(id) });
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

export function useDeleteAsset(projectId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.deleteAsset(id)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["assets", projectId] });
    },
  });
}

/**
 * Bind (or unbind) an asset to a shot. Backed by `asset.shot_id` (a single-
 * valued FK), so any prior binding is silently overwritten — UI callers are
 * expected to confirm overwrites first. Pass `shotId: null` to unbind.
 */
export function useAssignAssetToShot() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, shotId }: { id: string; shotId: string | null }) =>
      unwrap(commands.assignAssetToShot(id, shotId)),
    onSuccess: (asset) => {
      qc.invalidateQueries({ queryKey: ["assets", asset.project_id] });
      qc.invalidateQueries({ queryKey: assetKeys.detail(asset.id) });
    },
  });
}

/**
 * Register the project's root directory with the asset-protocol scope so
 * `convertFileSrc` URLs from the webview resolve. Idempotent on the Rust
 * side; safe to call on every project mount.
 */
export function useRegisterProjectAssetScope() {
  return useMutation({
    mutationFn: (projectId: string) =>
      unwrap(commands.registerProjectAssetScope(projectId)),
  });
}
