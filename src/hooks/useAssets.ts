import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type Asset,
  type AssetType,
  type ImportAssetInput,
} from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export const assetKeys = {
  all: (projectId: string, type: AssetType | "all") =>
    ["assets", projectId, type] as const,
  detail: (id: string) => ["asset", id] as const,
};

export function useAssetList(
  projectId: string | undefined,
  type?: AssetType,
) {
  const key = type ?? "all";
  return useQuery<Asset[]>({
    queryKey: projectId ? assetKeys.all(projectId, key) : ["assets", "none"],
    queryFn: () =>
      unwrap(
        commands.listAssets({
          project_id: projectId as string,
          asset_type: type ?? null,
        }),
      ),
    enabled: !!projectId,
  });
}

export function useAsset(id: string | undefined) {
  return useQuery<Asset>({
    queryKey: id ? assetKeys.detail(id) : ["asset", "none"],
    queryFn: () => unwrap(commands.getAsset(id as string)),
    enabled: !!id,
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
