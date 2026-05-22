import { useCallback } from "react";
import { ImageIcon, Upload, X } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { useImportAsset } from "@/hooks/useAssets";
import { useResolvedAssetUrl } from "@/hooks/useResolvedAssetUrl";
import { useGlobalDropTarget } from "@/hooks/useGlobalDropTarget";
import { commands } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

interface Props {
  projectId: string;
  projectRoot: string | undefined;
  currentRelativePath: string | null;
  onChange: (relativePath: string | null) => void;
  disabled?: boolean;
}

export function ReferenceImageDropzone({
  projectId,
  projectRoot,
  currentRelativePath,
  onChange,
  disabled = false,
}: Props) {
  const importAsset = useImportAsset();

  const handleImport = useCallback(
    async (sourcePath: string) => {
      try {
        const asset = await importAsset.mutateAsync({
          project_id: projectId,
          source_path: sourcePath,
        });
        onChange(asset.file_path);
      } catch (err) {
        toast.error(
          `导入失败：${err instanceof Error ? err.message : String(err)}`,
        );
      }
    },
    [projectId, onChange, importAsset],
  );

  const handleDrop = useCallback(
    (paths: string[]) => {
      const first = paths[0];
      if (first) void handleImport(first);
    },
    [handleImport],
  );

  useGlobalDropTarget(handleDrop, !disabled);

  const url = useResolvedAssetUrl(projectRoot, currentRelativePath);

  const handlePick = async () => {
    try {
      const picked = await unwrap(commands.pickImageFile());
      if (picked) void handleImport(picked);
    } catch (err) {
      toast.error(
        `选择文件失败：${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  return (
    <div className="flex flex-col gap-2">
      <div className="relative h-[200px] w-[200px] rounded border-2 border-dashed">
        {url ? (
          <img
            src={url}
            className="h-full w-full rounded object-cover"
            alt="参考图"
          />
        ) : (
          <div className="flex h-full flex-col items-center justify-center gap-2 p-3 text-center">
            <ImageIcon className="size-8 text-muted-foreground/40" />
            <p className="text-xs text-muted-foreground">
              拖入图片到窗口任意位置
              <br />或点击下方按钮选择
            </p>
          </div>
        )}
        {currentRelativePath && !disabled && (
          <button
            type="button"
            onClick={() => onChange(null)}
            className="absolute top-1 right-1 rounded-full bg-background/80 p-1 hover:bg-background"
            aria-label="清空参考图"
          >
            <X className="size-4" />
          </button>
        )}
      </div>
      <Button
        type="button"
        variant="outline"
        size="sm"
        onClick={handlePick}
        disabled={disabled || importAsset.isPending}
      >
        <Upload className="size-4" />
        {importAsset.isPending ? "导入中..." : "选择文件"}
      </Button>
    </div>
  );
}
