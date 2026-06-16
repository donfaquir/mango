import { useState } from "react";
import { Check, Pencil, Trash2 } from "lucide-react";
import type { Asset } from "@/lib/bindings/commands";
import { parseDbDate } from "@/lib/datetime";
import { useResolvedAssetUrl } from "@/hooks/useResolvedAssetUrl";
import {
  useUpdateAssetLabel,
  useUpdateAssetOriginalName,
} from "@/hooks/useAssets";
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
  const updateOriginalName = useUpdateAssetOriginalName();

  const [editingLabel, setEditingLabel] = useState(false);
  const [labelValue, setLabelValue] = useState("");
  const [editingName, setEditingName] = useState(false);
  const [nameValue, setNameValue] = useState("");

  if (!asset) return null;

  // metadata_json carries `{width, height, prompt?}` for generated images;
  // we surface the original prompt in a dedicated block so users can read
  // the full text regardless of how `original_name` ended up truncated for
  // display in cards. Malformed JSON degrades silently to "no prompt".
  let promptText: string | null = null;
  if (asset.metadata_json) {
    try {
      const parsed = JSON.parse(asset.metadata_json);
      if (typeof parsed?.prompt === "string" && parsed.prompt.trim().length > 0) {
        promptText = parsed.prompt;
      }
    } catch {
      // ignore
    }
  }

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

  const startEditName = () => {
    setNameValue(asset.original_name);
    setEditingName(true);
  };

  const saveName = () => {
    const trimmed = nameValue.trim();
    if (trimmed.length > 0 && trimmed !== asset.original_name) {
      updateOriginalName.mutate({ id: asset.id, originalName: trimmed });
    }
    setEditingName(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl max-h-[90vh] overflow-y-auto">
        <DialogHeader className="min-w-0">
          {/* `min-w-0` + `truncate` is the only combo that keeps the title
              from forcing the dialog wider than its `max-w-2xl` when the
              prompt text is long. Flex items default to `min-width: auto`,
              which silently defeats `truncate`. */}
          <DialogTitle className="truncate min-w-0" title={asset.original_name}>
            {asset.original_name}
          </DialogTitle>
          <DialogDescription>素材详情与预览</DialogDescription>
        </DialogHeader>

        {/* Editable display name. Sits below the title so the title still
            shows the canonical value while the user types. */}
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-sm text-muted-foreground shrink-0">名称：</span>
          {editingName ? (
            <>
              <Input
                value={nameValue}
                onChange={(e) => setNameValue(e.target.value)}
                className="h-8 flex-1 min-w-0"
                onKeyDown={(e) => {
                  if (e.key === "Enter") saveName();
                  if (e.key === "Escape") setEditingName(false);
                }}
                autoFocus
              />
              <Button size="sm" variant="ghost" onClick={saveName}>
                <Check className="h-4 w-4" />
              </Button>
            </>
          ) : (
            <>
              <span
                className="text-sm flex-1 truncate min-w-0"
                title={asset.original_name}
              >
                {asset.original_name}
              </span>
              <Button size="sm" variant="ghost" onClick={startEditName}>
                <Pencil className="h-3.5 w-3.5" />
              </Button>
            </>
          )}
        </div>

        {/* Media preview */}
        <div className="flex items-center justify-center rounded-md bg-muted overflow-hidden max-h-[400px]">
          {resolvedUrl ? (
            asset.asset_type === "video" ? (
              <video
                src={resolvedUrl}
                controls
                className="max-h-[400px] w-full object-contain"
              />
            ) : asset.asset_type === "audio" ? (
              <div className="flex w-full flex-col items-center justify-center gap-4 py-8">
                <audio src={resolvedUrl} controls autoPlay className="w-full max-w-md" />
              </div>
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

        {/* Full generation prompt — surfaced separately because cards
            truncate the display name and users often need to read the
            unabridged hint to remember what they asked for.
            `break-words` covers prompts without whitespace (long URLs,
            CJK without spaces) so the block wraps instead of overflowing. */}
        {promptText && (
          <div className="space-y-1 min-w-0">
            <span className="text-sm text-muted-foreground">生成提示词</span>
            <p className="whitespace-pre-wrap break-words rounded-md border bg-muted/30 px-3 py-2 text-xs text-foreground">
              {promptText}
            </p>
          </div>
        )}

        {/* Label editing */}
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-sm text-muted-foreground shrink-0">标签：</span>
          {editingLabel ? (
            <>
              <Input
                value={labelValue}
                onChange={(e) => setLabelValue(e.target.value)}
                className="h-8 flex-1 min-w-0"
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
              <span className="text-sm flex-1 truncate min-w-0" title={asset.label}>
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
