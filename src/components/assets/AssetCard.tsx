import { useState } from "react";
import { Film, Image, Music, Sparkles, Import, Trash2, Eye } from "lucide-react";
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
const TYPE_CONFIG: Record<Asset["asset_type"], { icon: typeof Film; label: string; className: string }> = {
  video: { icon: Film, label: "视频", className: "bg-purple-500/80 text-white" },
  image: { icon: Image, label: "图片", className: "bg-blue-500/80 text-white" },
  audio: { icon: Music, label: "音频", className: "bg-emerald-500/80 text-white" },
  script: { icon: Image, label: "脚本", className: "bg-gray-500/80 text-white" },
};

function TypeBadge({ type }: { type: Asset["asset_type"] }) {
  const cfg = TYPE_CONFIG[type] ?? TYPE_CONFIG.image;
  const Icon = cfg.icon;
  return (
    <span className={cn("inline-flex items-center gap-0.5 rounded px-1.5 py-0.5 text-[10px] font-medium", cfg.className)}>
      <Icon className="h-3 w-3" />
      {cfg.label}
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
        {asset.asset_type === "audio" ? (
          <div className="flex h-full w-full items-center justify-center px-3">
            {resolvedUrl ? (
              <audio
                src={resolvedUrl}
                controls
                preload="metadata"
                className="w-full"
                onClick={(e) => e.stopPropagation()}
              />
            ) : (
              <Music className="h-8 w-8 text-muted-foreground/50" />
            )}
          </div>
        ) : resolvedUrl && !imgError ? (
          asset.asset_type === "video" ? (
            <video
              src={resolvedUrl}
              className="h-full w-full object-cover"
              muted
              preload="metadata"
              draggable={false}
            />
          ) : (
            <img
              src={resolvedUrl}
              alt={asset.original_name}
              className="h-full w-full object-cover"
              onError={() => setImgError(true)}
              draggable={false}
            />
          )
        ) : (
          <div className="flex h-full w-full items-center justify-center">
            {(() => {
              const Icon = TYPE_CONFIG[asset.asset_type]?.icon ?? Image;
              return <Icon className="h-8 w-8 text-muted-foreground/50" />;
            })()}
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

      {/* Footer with filename. `title` makes the full string visible on hover
          even after CSS truncation — important now that `original_name` is
          the generation prompt full text, often ≫ card width. */}
      <div className="px-2 py-1.5">
        <p className="truncate text-xs text-foreground" title={asset.original_name}>
          {asset.original_name}
        </p>
      </div>
    </div>
  );
}
