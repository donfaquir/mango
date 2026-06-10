import { useEffect, useState } from "react";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetFooter,
} from "@/components/ui/sheet";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import { commands } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";
import { useFFmpegProgress } from "@/hooks/useFFmpegProgress";
import { toast } from "sonner";

interface ExportSettingsSheetProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  episodeId: string;
  defaultOutputPath: string;
}

export function ExportSettingsSheet({
  open,
  onOpenChange,
  episodeId,
  defaultOutputPath,
}: ExportSettingsSheetProps) {
  const [outputPath, setOutputPath] = useState(defaultOutputPath);
  const [exporting, setExporting] = useState(false);
  const [includeVoice, setIncludeVoice] = useState(true);
  const [includeSfx, setIncludeSfx] = useState(true);
  const [includeBgm, setIncludeBgm] = useState(true);

  useEffect(() => {
    setOutputPath(defaultOutputPath);
  }, [defaultOutputPath]);
  const { progress, reset: resetProgress } = useFFmpegProgress();

  const handleExport = async () => {
    setExporting(true);
    resetProgress();
    try {
      const result = await unwrap(
        commands.exportFinal(episodeId, outputPath, {
          include_voice: includeVoice,
          include_sfx: includeSfx,
          include_bgm: includeBgm,
        }),
      );
      toast.success(`导出完成：${result}`);
      onOpenChange(false);
    } catch (e) {
      toast.error(`导出失败：${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setExporting(false);
      resetProgress();
    }
  };

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" className="w-80">
        <SheetHeader>
          <SheetTitle>导出成片</SheetTitle>
        </SheetHeader>
        <div className="flex flex-col gap-4 py-4">
          <div className="space-y-2">
            <Label htmlFor="output-path">输出路径</Label>
            <Input
              id="output-path"
              value={outputPath}
              onChange={(e) => setOutputPath(e.target.value)}
              disabled={exporting}
            />
          </div>
          <div className="text-xs text-muted-foreground">
            格式：MP4 (H.264)，保持原始分辨率
          </div>

          <div className="space-y-2">
            <Label className="text-sm font-medium">音频轨</Label>
            <div className="flex flex-col gap-2">
              <label className="flex items-center gap-2 text-sm cursor-pointer">
                <input
                  type="checkbox"
                  checked={includeVoice}
                  onChange={(e) => setIncludeVoice(e.target.checked)}
                  disabled={exporting}
                  className="rounded border-input"
                />
                配音
              </label>
              <label className="flex items-center gap-2 text-sm cursor-pointer">
                <input
                  type="checkbox"
                  checked={includeSfx}
                  onChange={(e) => setIncludeSfx(e.target.checked)}
                  disabled={exporting}
                  className="rounded border-input"
                />
                音效
              </label>
              <label className="flex items-center gap-2 text-sm cursor-pointer">
                <input
                  type="checkbox"
                  checked={includeBgm}
                  onChange={(e) => setIncludeBgm(e.target.checked)}
                  disabled={exporting}
                  className="rounded border-input"
                />
                背景音乐
              </label>
            </div>
          </div>

          {exporting && progress && (
            <div className="space-y-1">
              <Progress value={progress.progress_pct ?? 0} className="h-2" />
              <div className="text-[10px] text-muted-foreground text-right">
                {(progress.progress_pct ?? 0).toFixed(0)}%
                {progress.speed != null && ` · ${progress.speed.toFixed(1)}x`}
              </div>
            </div>
          )}
        </div>
        <SheetFooter>
          <Button
            onClick={handleExport}
            disabled={exporting || !outputPath.trim()}
            className="w-full"
          >
            {exporting ? "导出中..." : "开始导出"}
          </Button>
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
}
