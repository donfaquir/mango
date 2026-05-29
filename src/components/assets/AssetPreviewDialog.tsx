import { useState } from "react";
import { Check, Pencil, Trash2 } from "lucide-react";
import type { Asset } from "@/lib/bindings/commands";
import { parseDbDate } from "@/lib/datetime";
import { useResolvedAssetUrl } from "@/hooks/useResolvedAssetUrl";
import { useUpdateAssetLabel } from "@/hooks/useAssets";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

interface AssetPreviewDialogProps {
  asset: Asset | null;
  projectRoot: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onDelete: (asset: Asset) => void;
}

// Format file size to human-readable string
function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatDate(iso: string): string {
  return parseDbDate(iso).toLocaleString("zh-CN");
}

const SOURCE_LABELS: Record<string, string> = {
  imported: "导入",
  generated: "AI 生成",
};

const TYPE_LABELS: Record<string, string> = {
  image: "图片",
  video: "视频",
  audio: "音频",
  script: "脚本",
};

export function AssetPreviewDialog({
  asset,
  projectRoot,
  open,
  onOpenChange,
  onDelete,
}: AssetPreviewDialogProps) {
  const resolvedUrl = useResolvedAssetUrl(projectRoot, asset?.file_path ?? null);
  const updateLabel = useUpdateAssetLabel();

  const [editingLabel, setEditingLabel] = useState(false);
  const [labelValue, setLabelValue] = useState("");

  if (!asset) return null;

  const startEditLabel = () => {
    setLabelValue(asset.label);
    setEditingLabel(true);
  };

  const saveLabel = () => {
    if (labelValue !== asset.label) {
      updateLabel.mutate({ id: asset.id, label: labelValue });
    }
    setEditingLabel(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="truncate">{asset.original_name}</DialogTitle>
          <DialogDescription>素材详情与预览</DialogDescription>
        </DialogHeader>

        {/* Media preview */}
        <div className="flex items-center justify-center rounded-md bg-muted overflow-hidden max-h-[400px]">
          {resolvedUrl ? (
            asset.asset_type === "video" ? (
              <video
                src={resolvedUrl}
                controls
                className="max-h-[400px] w-full object-contain"
              />
            ) : (
              <img
                src={resolvedUrl}
                alt={asset.original_name}
                className="max-h-[400px] w-full object-contain"
              />
            )
          ) : (
            <div className="flex h-48 w-full items-center justify-center text-muted-foreground">
              加载中...
            </div>
          )}
        </div>

        {/* Metadata info */}
        <div className="grid grid-cols-2 gap-x-4 gap-y-2 text-sm">
          <div>
            <span className="text-muted-foreground">类型：</span>
            {TYPE_LABELS[asset.asset_type] ?? asset.asset_type}
          </div>
          <div>
            <span className="text-muted-foreground">来源：</span>
            {SOURCE_LABELS[asset.source] ?? asset.source}
          </div>
          <div>
            <span className="text-muted-foreground">大小：</span>
            {formatFileSize(asset.file_size)}
          </div>
          <div>
            <span className="text-muted-foreground">创建时间：</span>
            {formatDate(asset.created_at)}
          </div>
        </div>

        {/* Label editing */}
        <div className="flex items-center gap-2">
          <span className="text-sm text-muted-foreground shrink-0">标签：</span>
          {editingLabel ? (
            <>
              <Input
                value={labelValue}
                onChange={(e) => setLabelValue(e.target.value)}
                className="h-8 flex-1"
                onKeyDown={(e) => {
                  if (e.key === "Enter") saveLabel();
                  if (e.key === "Escape") setEditingLabel(false);
                }}
                autoFocus
              />
              <Button size="sm" variant="ghost" onClick={saveLabel}>
                <Check className="h-4 w-4" />
              </Button>
            </>
          ) : (
            <>
              <span className="text-sm flex-1">
                {asset.label || <span className="text-muted-foreground italic">无标签</span>}
              </span>
              <Button size="sm" variant="ghost" onClick={startEditLabel}>
                <Pencil className="h-3.5 w-3.5" />
              </Button>
            </>
          )}
        </div>

        <DialogFooter>
          <Button
            variant="destructive"
            onClick={() => {
              onDelete(asset);
              onOpenChange(false);
            }}
          >
            <Trash2 className="mr-1 h-4 w-4" />
            删除素材
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
