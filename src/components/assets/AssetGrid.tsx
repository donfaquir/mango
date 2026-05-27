import { ImageOff } from "lucide-react";
import type { Asset } from "@/lib/bindings/commands";
import { AssetCard } from "./AssetCard";
import { EmptyState } from "@/components/common/EmptyState";
import { Skeleton } from "@/components/ui/skeleton";

interface AssetGridProps {
  assets: Asset[] | undefined;
  projectRoot: string;
  isLoading: boolean;
  onPreview: (asset: Asset) => void;
  onDelete: (asset: Asset) => void;
}

export function AssetGrid({
  assets,
  projectRoot,
  isLoading,
  onPreview,
  onDelete,
}: AssetGridProps) {
  if (isLoading) {
    return (
      <div className="grid grid-cols-2 gap-3 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
        {Array.from({ length: 10 }).map((_, i) => (
          <div key={i} className="flex flex-col overflow-hidden rounded-lg border">
            <Skeleton className="aspect-square w-full" />
            <Skeleton className="mx-2 my-1.5 h-4 w-3/4" />
          </div>
        ))}
      </div>
    );
  }

  if (!assets || assets.length === 0) {
    return (
      <EmptyState
        icon={ImageOff}
        title="暂无素材"
        description="当前筛选条件下没有找到任何素材"
      />
    );
  }

  return (
    <div className="grid grid-cols-2 gap-3 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
      {assets.map((asset) => (
        <AssetCard
          key={asset.id}
          asset={asset}
          projectRoot={projectRoot}
          onPreview={onPreview}
          onDelete={onDelete}
        />
      ))}
    </div>
  );
}
