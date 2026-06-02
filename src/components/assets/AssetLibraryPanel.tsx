import { useCallback, useMemo, useState } from "react";
import type { Asset } from "@/lib/bindings/commands";
import {
  useAssetLabels,
  useAssetList,
  useScheduleAssetDeletion,
} from "@/hooks/useAssets";
import { AssetFilterBar, type AssetFilterValues } from "./AssetFilterBar";
import { AssetGrid } from "./AssetGrid";
import { AssetPreviewDialog } from "./AssetPreviewDialog";
import { DeleteAssetConfirmDialog } from "./DeleteAssetConfirmDialog";

interface AssetLibraryPanelProps {
  projectId: string;
  projectRoot: string;
}

export function AssetLibraryPanel({ projectId, projectRoot }: AssetLibraryPanelProps) {
  const [filters, setFilters] = useState<AssetFilterValues>({
    type: undefined,
    source: undefined,
    keyword: "",
    label: undefined,
  });

  const [previewAssetId, setPreviewAssetId] = useState<string | null>(null);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [pendingConfirmAsset, setPendingConfirmAsset] = useState<Asset | null>(
    null,
  );
  const [confirmOpen, setConfirmOpen] = useState(false);

  const { data: assets, isLoading } = useAssetList(projectId, filters.type, {
    source: filters.source,
    keyword: filters.keyword || undefined,
    label: filters.label,
  });
  const { data: labels } = useAssetLabels(projectId);

  // Re-derive the previewed asset from the (cache-backed) list so label/shot
  // edits made inside the dialog show up without reopening it.
  const previewAsset = useMemo<Asset | null>(
    () => assets?.find((a) => a.id === previewAssetId) ?? null,
    [assets, previewAssetId],
  );

  const scheduleDelete = useScheduleAssetDeletion(projectId);

  const handlePreview = useCallback((asset: Asset) => {
    setPreviewAssetId(asset.id);
    setPreviewOpen(true);
  }, []);

  const handleRequestDelete = useCallback((asset: Asset) => {
    setPendingConfirmAsset(asset);
    setConfirmOpen(true);
  }, []);

  const handleConfirmedDelete = useCallback(
    (asset: Asset) => {
      // Close the preview if the user just confirmed deletion of the previewed
      // asset — the optimistic cache update will make it disappear from the
      // grid, but the open dialog would still be sitting on a stale row.
      if (previewAssetId === asset.id) {
        setPreviewOpen(false);
        setPreviewAssetId(null);
      }
      scheduleDelete(asset);
      setPendingConfirmAsset(null);
    },
    [scheduleDelete, previewAssetId],
  );

  return (
    <div className="flex flex-col gap-4">
      <header className="flex items-center justify-between">
        <h2 className="text-xl font-semibold">素材库</h2>
      </header>

      <AssetFilterBar value={filters} labels={labels ?? []} onChange={setFilters} />

      <AssetGrid
        assets={assets}
        projectRoot={projectRoot}
        isLoading={isLoading}
        onPreview={handlePreview}
        onDelete={handleRequestDelete}
      />

      <AssetPreviewDialog
        asset={previewAsset}
        projectRoot={projectRoot}
        open={previewOpen}
        onOpenChange={setPreviewOpen}
        onDelete={handleRequestDelete}
      />

      <DeleteAssetConfirmDialog
        asset={pendingConfirmAsset}
        open={confirmOpen}
        onOpenChange={setConfirmOpen}
        onConfirm={handleConfirmedDelete}
      />
    </div>
  );
}
