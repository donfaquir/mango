import { useState, useCallback, useEffect } from "react";
import { Search, Upload, ImageIcon, Loader2 } from "lucide-react";
import { toast } from "sonner";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useAssetList, useImportAsset } from "@/hooks/useAssets";
import { useResolvedAssetUrl } from "@/hooks/useResolvedAssetUrl";
import { commands, type Asset, type AssetSource, type AssetType } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";
import { cn } from "@/lib/utils";

interface AssetPickerDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  projectId: string;
  projectRoot: string | undefined;
  assetType?: "image" | "video" | "audio";
  onSelect: (asset: Asset) => void;
}

// Individual thumbnail cell that resolves asset URL
function AssetThumbnailCell({
  asset,
  projectRoot,
  selected,
  onClick,
  onDoubleClick,
}: {
  asset: Asset;
  projectRoot: string | undefined;
  selected: boolean;
  onClick: () => void;
  onDoubleClick: () => void;
}) {
  const thumbPath = asset.thumbnail_path ?? asset.file_path;
  const url = useResolvedAssetUrl(projectRoot, thumbPath);

  return (
    <button
      type="button"
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      className={cn(
        "relative flex flex-col gap-1.5 rounded-lg border p-2 text-left transition-colors hover:bg-accent",
        selected && "border-primary ring-2 ring-primary/20",
      )}
    >
      <div className="relative aspect-video w-full overflow-hidden rounded bg-muted">
        {url ? (
          asset.asset_type === "video" ? (
            <video
              src={url}
              className="h-full w-full object-cover"
              muted
              preload="metadata"
            />
          ) : (
            <img
              src={url}
              className="h-full w-full object-cover"
              alt={asset.original_name}
            />
          )
        ) : (
          <div className="flex h-full items-center justify-center">
            <ImageIcon className="size-6 text-muted-foreground/40" />
          </div>
        )}
      </div>
      <span className="truncate text-xs text-muted-foreground">
        {asset.label || asset.original_name}
      </span>
    </button>
  );
}

export function AssetPickerDialog({
  open,
  onOpenChange,
  projectId,
  projectRoot,
  assetType,
  onSelect,
}: AssetPickerDialogProps) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [source, setSource] = useState<AssetSource | undefined>(undefined);
  const [keyword, setKeyword] = useState("");
  const [searchInput, setSearchInput] = useState("");

  const importAsset = useImportAsset();

  // Reset selection state when dialog opens
  useEffect(() => {
    if (open) {
      setSelectedId(null);
      setSource(undefined);
      setKeyword("");
      setSearchInput("");
    }
  }, [open]);

  // Debounce keyword search
  useEffect(() => {
    const timer = setTimeout(() => setKeyword(searchInput), 300);
    return () => clearTimeout(timer);
  }, [searchInput]);

  const { data: assets, isLoading } = useAssetList(
    open ? projectId : undefined,
    assetType as AssetType | undefined,
    { source, keyword: keyword || undefined },
  );

  const selectedAsset = assets?.find((a) => a.id === selectedId) ?? null;

  const handleConfirm = useCallback(() => {
    if (selectedAsset) {
      onSelect(selectedAsset);
      onOpenChange(false);
    }
  }, [selectedAsset, onSelect, onOpenChange]);

  const handleDoubleClick = useCallback(
    (asset: Asset) => {
      onSelect(asset);
      onOpenChange(false);
    },
    [onSelect, onOpenChange],
  );

  const handleImportLocal = useCallback(async () => {
    try {
      // Use native file picker
      const picked = assetType && assetType !== "image"
        ? await unwrap(commands.pickMediaFile(assetType))
        : await unwrap(commands.pickImageFile());
      if (!picked) return;

      const imported = await importAsset.mutateAsync({
        project_id: projectId,
        source_path: picked,
      });
      // Auto-select the newly imported asset
      setSelectedId(imported.id);
    } catch (err) {
      toast.error(
        `导入失败：${err instanceof Error ? err.message : String(err)}`,
      );
    }
  }, [projectId, importAsset]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-3xl">
        <DialogTitle>选择素材</DialogTitle>

        {/* Filter bar */}
        <div className="flex items-center gap-3">
          {/* Source filter */}
          <div className="flex items-center gap-1 rounded-md border p-0.5">
            <button
              type="button"
              onClick={() => setSource(undefined)}
              className={cn(
                "rounded px-2.5 py-1 text-xs font-medium transition-colors",
                source === undefined
                  ? "bg-primary text-primary-foreground"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              全部
            </button>
            <button
              type="button"
              onClick={() => setSource("imported")}
              className={cn(
                "rounded px-2.5 py-1 text-xs font-medium transition-colors",
                source === "imported"
                  ? "bg-primary text-primary-foreground"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              导入
            </button>
            <button
              type="button"
              onClick={() => setSource("generated")}
              className={cn(
                "rounded px-2.5 py-1 text-xs font-medium transition-colors",
                source === "generated"
                  ? "bg-primary text-primary-foreground"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              生成
            </button>
          </div>

          {/* Search input */}
          <div className="relative flex-1">
            <Search className="absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              placeholder="搜索素材..."
              value={searchInput}
              onChange={(e) => setSearchInput(e.target.value)}
              className="pl-8 h-8"
            />
          </div>
        </div>

        {/* Asset grid */}
        <div className="max-h-[55vh] min-h-[200px] overflow-auto">
          {isLoading ? (
            <div className="flex h-[200px] items-center justify-center">
              <Loader2 className="size-6 animate-spin text-muted-foreground" />
            </div>
          ) : !assets || assets.length === 0 ? (
            <div className="flex h-[200px] flex-col items-center justify-center gap-2 text-center">
              <ImageIcon className="size-10 text-muted-foreground/30" />
              <p className="text-sm text-muted-foreground">
                暂无可选素材，请点击下方"从本地导入"添加
              </p>
            </div>
          ) : (
            <div className="grid grid-cols-3 gap-3 sm:grid-cols-4">
              {assets.map((asset) => (
                <AssetThumbnailCell
                  key={asset.id}
                  asset={asset}
                  projectRoot={projectRoot}
                  selected={asset.id === selectedId}
                  onClick={() => setSelectedId(asset.id)}
                  onDoubleClick={() => handleDoubleClick(asset)}
                />
              ))}
            </div>
          )}
        </div>

        {/* Footer actions */}
        <DialogFooter className="sm:justify-between">
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={handleImportLocal}
            disabled={importAsset.isPending}
          >
            <Upload className="size-4" />
            {importAsset.isPending ? "导入中..." : "从本地导入"}
          </Button>
          <div className="flex gap-2">
            <Button variant="outline" onClick={() => onOpenChange(false)}>
              取消
            </Button>
            <Button onClick={handleConfirm} disabled={!selectedAsset}>
              确认选择
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
