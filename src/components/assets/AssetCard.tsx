import { useState } from "react";
import { Film, Image, Sparkles, Import, Trash2, Eye } from "lucide-react";
import type { Asset } from "@/lib/bindings/commands";
import { useResolvedAssetUrl } from "@/hooks/useResolvedAssetUrl";
import { cn } from "@/lib/utils";

interface AssetCardProps {
  asset: Asset;
  projectRoot: string;
  onPreview: (asset: Asset) => void;
  onDelete: (asset: Asset) => void;
}

// Badge for asset type
function TypeBadge({ type }: { type: Asset["asset_type"] }) {
  const isVideo = type === "video";
  return (
    <span
      className={cn(
        "inline-flex items-center gap-0.5 rounded px-1.5 py-0.5 text-[10px] font-medium",
        isVideo
          ? "bg-purple-500/80 text-white"
          : "bg-blue-500/80 text-white",
      )}
    >
      {isVideo ? <Film className="h-3 w-3" /> : <Image className="h-3 w-3" />}
      {isVideo ? "视频" : "图片"}
    </span>
  );
}

// Badge for asset source
function SourceBadge({ source }: { source: Asset["source"] }) {
  const isGenerated = source === "generated";
  return (
    <span
      className={cn(
        "inline-flex items-center gap-0.5 rounded px-1.5 py-0.5 text-[10px] font-medium",
        isGenerated
          ? "bg-amber-500/80 text-white"
          : "bg-green-500/80 text-white",
      )}
    >
      {isGenerated ? <Sparkles className="h-3 w-3" /> : <Import className="h-3 w-3" />}
      {isGenerated ? "生成" : "导入"}
    </span>
  );
}

export function AssetCard({ asset, projectRoot, onPreview, onDelete }: AssetCardProps) {
  const [imgError, setImgError] = useState(false);

  // Prefer thumbnail_path, fallback to file_path
  const displayPath = asset.thumbnail_path ?? asset.file_path;
  const resolvedUrl = useResolvedAssetUrl(projectRoot, displayPath);

  return (
    <div
      className="group relative flex flex-col overflow-hidden rounded-lg border bg-card transition-shadow hover:shadow-md cursor-pointer"
      onClick={() => onPreview(asset)}
    >
      {/* Thumbnail area */}
      <div className="relative aspect-square overflow-hidden bg-muted">
        {resolvedUrl && !imgError ? (
          asset.asset_type === "video" ? (
            <video
              src={resolvedUrl}
              className="h-full w-full object-cover"
              muted
              preload="metadata"
            />
          ) : (
            <img
              src={resolvedUrl}
              alt={asset.original_name}
              className="h-full w-full object-cover"
              onError={() => setImgError(true)}
            />
          )
        ) : (
          <div className="flex h-full w-full items-center justify-center">
            {asset.asset_type === "video" ? (
              <Film className="h-8 w-8 text-muted-foreground/50" />
            ) : (
              <Image className="h-8 w-8 text-muted-foreground/50" />
            )}
          </div>
        )}

        {/* Badges */}
        <div className="absolute top-1.5 left-1.5 flex flex-col gap-1">
          <TypeBadge type={asset.asset_type} />
          <SourceBadge source={asset.source} />
        </div>

        {/* Hover overlay with actions */}
        <div className="absolute inset-0 flex items-center justify-center gap-2 bg-black/40 opacity-0 transition-opacity group-hover:opacity-100">
          <button
            className="rounded-full bg-white/90 p-2 text-foreground shadow-sm hover:bg-white"
            onClick={(e) => {
              e.stopPropagation();
              onPreview(asset);
            }}
            title="预览"
          >
            <Eye className="h-4 w-4" />
          </button>
          <button
            className="rounded-full bg-white/90 p-2 text-destructive shadow-sm hover:bg-white"
            onClick={(e) => {
              e.stopPropagation();
              onDelete(asset);
            }}
            title="删除"
          >
            <Trash2 className="h-4 w-4" />
          </button>
        </div>
      </div>

      {/* Footer with filename */}
      <div className="px-2 py-1.5">
        <p className="truncate text-xs text-foreground" title={asset.original_name}>
          {asset.original_name}
        </p>
      </div>
    </div>
  );
}
