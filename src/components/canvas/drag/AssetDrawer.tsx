import { useState } from "react";
import { FolderOpen, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { Asset } from "@/lib/bindings/commands";
import { AssetCard } from "@/components/assets/AssetCard";
import {
  AssetFilterBar,
  type AssetFilterValues,
} from "@/components/assets/AssetFilterBar";
import { useAssetLabels, useAssetList } from "@/hooks/useAssets";
import { DRAG_MIME_ASSET_ID } from "./constants";

interface AssetDrawerProps {
  projectId: string;
  projectRoot: string;
}

// Wrap AssetCard with a draggable div so the drag behavior is scoped to the
// drawer only — AssetCard itself is also rendered on the asset library page,
// where `draggable` would interfere with click/selection.
function DraggableAssetCard({
  asset,
  projectRoot,
}: {
  asset: Asset;
  projectRoot: string;
}) {
  const noop = () => {};
  return (
    <div
      draggable
      onDragStart={(e) => {
        e.dataTransfer.setData(DRAG_MIME_ASSET_ID, asset.id);
        e.dataTransfer.effectAllowed = "copy";
      }}
      className="cursor-grab active:cursor-grabbing"
    >
      <AssetCard
        asset={asset}
        projectRoot={projectRoot}
        onPreview={noop}
        onDelete={noop}
      />
    </div>
  );
}

export function AssetDrawer({ projectId, projectRoot }: AssetDrawerProps) {
  const [open, setOpen] = useState(false);
  const [filters, setFilters] = useState<AssetFilterValues>({
    type: undefined,
    source: undefined,
    keyword: "",
    label: undefined,
  });

  const { data: assets } = useAssetList(projectId, filters.type, {
    source: filters.source,
    keyword: filters.keyword || undefined,
    label: filters.label,
  });
  const { data: labels } = useAssetLabels(projectId);

  return (
    <>
      <Button
        size="icon"
        variant="outline"
        className="absolute right-4 top-4 z-30"
        onClick={() => setOpen((v) => !v)}
        title="素材库"
        aria-label="切换素材库抽屉"
      >
        <FolderOpen className="size-4" />
      </Button>
      <aside
        className={cn(
          "absolute right-0 top-0 z-20 flex h-full w-80 flex-col border-l bg-background shadow-lg transition-transform",
          open ? "translate-x-0" : "pointer-events-none translate-x-full",
        )}
        aria-hidden={!open}
      >
        <header className="flex items-center justify-between border-b p-3">
          <h3 className="text-sm font-medium">素材库</h3>
          <Button
            size="icon"
            variant="ghost"
            onClick={() => setOpen(false)}
            aria-label="关闭素材库抽屉"
          >
            <X className="size-4" />
          </Button>
        </header>
        <div className="border-b p-3">
          <AssetFilterBar value={filters} labels={labels ?? []} onChange={setFilters} />
        </div>
        <div className="grid flex-1 grid-cols-2 gap-2 overflow-auto p-3">
          {assets?.map((asset) => (
            <DraggableAssetCard
              key={asset.id}
              asset={asset}
              projectRoot={projectRoot}
            />
          ))}
        </div>
      </aside>
    </>
  );
}
