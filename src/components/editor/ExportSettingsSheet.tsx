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

  useEffect(() => {
    setOutputPath(defaultOutputPath);
  }, [defaultOutputPath]);
  const { progress, reset: resetProgress } = useFFmpegProgress();

  const handleExport = async () => {
    setExporting(true);
    resetProgress();
    try {
      const result = await unwrap(
        commands.exportTimeline(episodeId, outputPath, {
          video_codec: "libx264",
          preset: "fast",
          crf: 18,
          audio_bitrate: "128k",
          container: "mp4",
          output_width: null,
          output_height: null,
          output_fps: null,
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
            格式：MP4 (H.264)，导出时间轴上的视频和音频轨道
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
