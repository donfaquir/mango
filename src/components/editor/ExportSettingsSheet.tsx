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
import { Slider } from "@/components/ui/slider";
import { commands, type RenderConfig } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";
import { useFFmpegProgress } from "@/hooks/useFFmpegProgress";
import { ExportConfigForm } from "./ExportConfigForm";
import { toast } from "sonner";

const DEFAULT_CONFIG: RenderConfig = {
  video_codec: "libx264",
  preset: "fast",
  crf: 18,
  audio_bitrate: "128k",
  container: "mp4",
  output_width: null,
  output_height: null,
  output_fps: null,
  fit_mode: null,
};

interface ExportSettingsSheetProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  episodeId: string;
  defaultOutputPath: string;
  totalDurationMs: number;
}

export function ExportSettingsSheet({
  open,
  onOpenChange,
  episodeId,
  defaultOutputPath,
  totalDurationMs,
}: ExportSettingsSheetProps) {
  const [outputPath, setOutputPath] = useState(defaultOutputPath);
  const [renderConfig, setRenderConfig] = useState<RenderConfig>(DEFAULT_CONFIG);
  const [exporting, setExporting] = useState(false);
  const [coverTimeMs, setCoverTimeMs] = useState(0);
  const [coverPath, setCoverPath] = useState("");
  const [extractingCover, setExtractingCover] = useState(false);

  useEffect(() => {
    setOutputPath(defaultOutputPath);
  }, [defaultOutputPath]);
  const { progress, reset: resetProgress } = useFFmpegProgress();

  const handleExport = async () => {
    setExporting(true);
    resetProgress();
    try {
      const result = await unwrap(
        commands.exportTimeline(episodeId, outputPath, renderConfig),
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

  const handleExtractCover = async () => {
    if (!outputPath.trim()) return;
    const coverOut = outputPath.replace(/\.\w+$/, "_cover.jpg");
    setExtractingCover(true);
    try {
      const result = await unwrap(
        commands.extractThumbnail(outputPath, coverTimeMs, coverOut),
      );
      setCoverPath(result);
      toast.success("封面帧已提取");
    } catch (e) {
      toast.error(`提取封面失败：${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setExtractingCover(false);
    }
  };

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" className="w-96 overflow-y-auto">
        <SheetHeader>
          <SheetTitle>导出成片</SheetTitle>
        </SheetHeader>
        <div className="flex flex-col gap-4 py-4">
          <ExportConfigForm
            config={renderConfig}
            onChange={setRenderConfig}
            disabled={exporting}
          />

          <div className="space-y-1.5">
            <Label className="text-xs">封面帧</Label>
            <div className="flex items-center gap-2">
              <Slider
                className="flex-1"
                min={0}
                max={Math.max(totalDurationMs, 1)}
                step={100}
                value={[coverTimeMs]}
                onValueChange={([v]) => setCoverTimeMs(v)}
                disabled={exporting}
              />
              <span className="text-xs text-muted-foreground font-mono w-14 text-right">
                {(coverTimeMs / 1000).toFixed(1)}s
              </span>
              <Button
                variant="outline"
                size="sm"
                className="h-6 text-xs"
                onClick={handleExtractCover}
                disabled={exporting || extractingCover || !outputPath.trim()}
              >
                {extractingCover ? "提取中..." : "提取封面"}
              </Button>
            </div>
            {coverPath && (
              <div className="text-[10px] text-muted-foreground truncate">{coverPath}</div>
            )}
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="output-path" className="text-xs">输出路径</Label>
            <Input
              id="output-path"
              value={outputPath}
              onChange={(e) => setOutputPath(e.target.value)}
              disabled={exporting}
              className="h-7 text-xs"
            />
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
