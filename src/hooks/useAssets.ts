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
      qc.invalidateQueries({ queryKey: assetKeys.labels(projectId) });
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
