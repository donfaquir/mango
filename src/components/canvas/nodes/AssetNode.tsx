import type { NodeProps } from "@xyflow/react";
import { useState } from "react";
import { ImageIcon, Play, Music, FileText } from "lucide-react";
import { useAsset } from "@/hooks/useAssets";
import { useResolvedAssetUrl } from "@/hooks/useResolvedAssetUrl";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import type { AssetNode as AssetNodeType } from "./types";
import { NodeShell } from "./shared/NodeShell";
import { useCurrentProjectRoot } from "./shared/useCurrentProjectRoot";

export function AssetNode({ data, selected }: NodeProps<AssetNodeType>) {
  const { data: asset } = useAsset(data.assetId);
  const projectRoot = useCurrentProjectRoot();
  const previewPath = asset?.thumbnail_path ?? asset?.file_path ?? null;
  const previewUrl = useResolvedAssetUrl(projectRoot, previewPath);
  const fileUrl = useResolvedAssetUrl(projectRoot, asset?.file_path ?? null);
  const [playOpen, setPlayOpen] = useState(false);

  const Icon = (() => {
    switch (asset?.asset_type) {
      case "audio":
        return Music;
      case "script":
        return FileText;
      default:
        return ImageIcon;
    }
  })();

  return (
    <NodeShell selected={selected} width={220}>
      <div className="flex items-center gap-2 border-b px-3 py-2 text-sm font-medium">
        <Icon className="size-4 text-muted-foreground" />
        <span className="truncate">
          {asset?.label || asset?.original_name || "素材"}
        </span>
      </div>
      <div className="relative bg-muted/40 p-2">
        {previewUrl ? (
          <img
            src={previewUrl}
            alt={asset?.original_name ?? ""}
            className="h-24 w-full rounded object-cover"
          />
        ) : (
          <div className="flex h-24 w-full items-center justify-center rounded bg-muted">
            <Icon className="size-8 text-muted-foreground/40" />
          </div>
        )}
        {asset?.asset_type === "video" && (
          <Button
            type="button"
            variant="secondary"
            size="sm"
            className="absolute bottom-3 right-3 h-7 gap-1"
            onClick={() => setPlayOpen(true)}
          >
            <Play className="size-3" />
            播放
          </Button>
        )}
      </div>
      {asset?.asset_type === "video" && (
        <Dialog open={playOpen} onOpenChange={setPlayOpen}>
          <DialogContent className="sm:max-w-4xl">
            {fileUrl ? (
              <video
                src={fileUrl}
                controls
                autoPlay
                className="max-h-[80vh] w-full"
              />
            ) : (
              <p className="p-6 text-center text-sm text-muted-foreground">
                加载中…
              </p>
            )}
          </DialogContent>
        </Dialog>
      )}
    </NodeShell>
  );
}
