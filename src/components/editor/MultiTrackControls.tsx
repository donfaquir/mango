import { useState, useCallback, useMemo } from "react";
import { join } from "@tauri-apps/api/path";
import { Download, Plus, Video, Music, Type, Play, Pause, SkipBack, SkipForward, Undo2, Redo2, Smile, Sparkles } from "lucide-react";
import { toast } from "sonner";
import { Slider } from "@/components/ui/slider";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { AssetPickerDialog } from "@/components/assets/AssetPickerDialog";
import { AddTextDialog, type TextType } from "./AddTextDialog";
import { StickerPicker } from "./StickerPicker";
import { ColorPresetPicker } from "./ColorPresetPicker";
import { useMultiTrackStore } from "@/stores/multiTrackStore";
import { commands, type Asset } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

const DEFAULT_TEXT_STYLES: Record<TextType, Record<string, unknown>> = {
  subtitle: { font_size: 36, color: "#FFFFFF", outline_color: "#000000", outline_width: 2, position_y: 0.9, alignment: "center" },
  bubble: { font_size: 28, color: "#000000", position_x: 0.5, position_y: 0.5 },
  fancy: { font_size: 72, font_weight: "bold", color: "#FFD700", outline_color: "#FF4500", outline_width: 3, position_y: 0.3, alignment: "center" },
  onomatopoeia: { font_size: 96, font_weight: "bold", color: "#FF0000", outline_color: "#000000", outline_width: 4, position_x: 0.5, position_y: 0.4 },
};

function formatTime(ms: number): string {
  const min = Math.floor(ms / 60_000);
  const sec = Math.floor((ms % 60_000) / 1000);
  const frac = Math.floor((ms % 1000) / 10);
  return `${min}:${sec.toString().padStart(2, "0")}.${frac.toString().padStart(2, "0")}`;
}

interface MultiTrackControlsProps {
  episodeId: string;
  projectId: string;
  projectRoot: string | undefined;
}

export function MultiTrackControls({ episodeId, projectId, projectRoot }: MultiTrackControlsProps) {
  const {
    playhead, zoom, setZoom, totalDuration, importing, items, tracks, selection,
    addItem, addTrack, updateItem, importFromShots, isPlaying, togglePlay, pause, setPlayhead,
  } = useMultiTrackStore();
  const [pickerOpen, setPickerOpen] = useState(false);
  const [pickerType, setPickerType] = useState<"video" | "audio">("video");
  const [textDialogOpen, setTextDialogOpen] = useState(false);
  const [stickerPickerOpen, setStickerPickerOpen] = useState(false);
  const [colorPresetPickerOpen, setColorPresetPickerOpen] = useState(false);

  const itemCount = Object.keys(items).length;

  const selectedTransition = useMemo(() => {
    if (selection.size !== 1) return null;
    const id = [...selection][0];
    const item = items[id];
    return item?.item_type === "transition" ? item : null;
  }, [selection, items]);

  const selectedStickerItem = useMemo(() => {
    if (selection.size !== 1) return null;
    const id = [...selection][0];
    const item = items[id];
    return item?.item_type === "sticker" ? item : null;
  }, [selection, items]);

  const handleImportFromShots = useCallback(async () => {
    if (itemCount > 0) {
      const ok = window.confirm(`时间轴已有 ${itemCount} 个片段，导入会在现有片段基础上追加。是否继续？`);
      if (!ok) return;
    }
    try {
      await importFromShots(episodeId);
      toast.success("镜头导入完成");
    } catch (err) {
      toast.error(`导入失败：${err instanceof Error ? err.message : String(err)}`);
    }
  }, [episodeId, itemCount, importFromShots]);

  const openPicker = useCallback((type: "video" | "audio") => {
    setPickerType(type);
    setPickerOpen(true);
  }, []);

  const handleAddText = useCallback(async (content: string, textType: TextType) => {
    let textTrack = tracks.find((t) => t.track_type === "text");
    if (!textTrack) {
      await addTrack({ episode_id: episodeId, track_type: "text", label: "文本" });
      textTrack = useMultiTrackStore.getState().tracks.find((t) => t.track_type === "text");
      if (!textTrack) return;
    }
    try {
      await addItem({
        track_id: textTrack.id,
        asset_id: null,
        item_type: "text",
        position_ms: playhead,
        duration_ms: 5000,
        in_point_ms: null,
        out_point_ms: 5000,
        params_json: JSON.stringify({
          content,
          text_type: textType,
          style: DEFAULT_TEXT_STYLES[textType],
          ...(textType === "bubble" ? { bubble: { shape: "rect", fill_color: "#FFFFFF", border_color: "#000000" } } : {}),
        }),
      });
      toast.success("文本已添加到时间轴");
    } catch (err) {
      toast.error(`添加文本失败：${err instanceof Error ? err.message : String(err)}`);
    }
  }, [tracks, playhead, addItem, addTrack, episodeId]);

  const addOverlayItem = useCallback(async (itemType: "sticker" | "effect", durationMs: number, paramsJson: string, label: string) => {
    let overlayTrack = tracks.find((t) => t.track_type === "overlay");
    if (!overlayTrack) {
      await addTrack({ episode_id: episodeId, track_type: "overlay", label: "贴片/特效" });
      overlayTrack = useMultiTrackStore.getState().tracks.find((t) => t.track_type === "overlay");
      if (!overlayTrack) return;
    }
    try {
      await addItem({
        track_id: overlayTrack.id, asset_id: null, item_type: itemType,
        position_ms: playhead, duration_ms: durationMs,
        in_point_ms: null, out_point_ms: durationMs, params_json: paramsJson,
      });
      toast.success(`${label}已添加到时间轴`);
    } catch (err) {
      toast.error(`添加${label}失败：${err instanceof Error ? err.message : String(err)}`);
    }
  }, [tracks, playhead, addItem, addTrack, episodeId]);

  const handleAssetSelected = useCallback(async (asset: Asset) => {
    const targetTrack = tracks.find((t) =>
      pickerType === "video" ? t.track_type === "video" : t.track_type === "audio",
    );
    if (!targetTrack) {
      toast.error("未找到匹配的轨道");
      return;
    }

    let durationMs = 5000;
    if (projectRoot) {
      try {
        const absPath = await join(projectRoot, asset.file_path);
        if (asset.asset_type === "video") {
          const meta = await unwrap(commands.probeVideo(absPath));
          durationMs = meta.duration_ms;
        } else if (asset.asset_type === "audio") {
          const dur = await unwrap(commands.probeAudioDuration(absPath));
          durationMs = dur.durationMs;
        }
      } catch {
        // fallback to default duration
      }
    }

    try {
      await addItem({
        track_id: targetTrack.id,
        asset_id: asset.id,
        item_type: "clip",
        position_ms: playhead,
        duration_ms: durationMs,
        in_point_ms: 0,
        out_point_ms: durationMs,
        params_json: "{}",
      });
      toast.success("媒体已添加到时间轴");
    } catch (err) {
      toast.error(`添加媒体失败：${err instanceof Error ? err.message : String(err)}`);
    }
  }, [tracks, pickerType, playhead, addItem, projectRoot]);

  return (
    <>
      <div className="flex items-center gap-2 px-3 py-1.5 border-t border-border bg-muted/20 text-xs">
        <Button
          variant="outline"
          size="sm"
          onClick={handleImportFromShots}
          disabled={importing}
          className="h-6 text-xs"
        >
          <Download className="size-3.5" />
          {importing ? "导入中..." : "导入镜头"}
        </Button>

        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="outline" size="sm" className="h-6 text-xs">
              <Plus className="size-3.5" />
              添加媒体
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent>
            <DropdownMenuItem onClick={() => openPicker("video")}>
              <Video className="size-4" />
              添加视频
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => openPicker("audio")}>
              <Music className="size-4" />
              添加音频
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => setTextDialogOpen(true)}>
              <Type className="size-4" />
              添加文本
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => setStickerPickerOpen(true)}>
              <Smile className="size-4" />
              添加贴纸
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => setColorPresetPickerOpen(true)}>
              <Sparkles className="size-4" />
              添加调色
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>

        <div className="flex items-center gap-0.5 ml-2">
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6"
            disabled={useMultiTrackStore.temporal.getState().pastStates.length === 0}
            onClick={() => useMultiTrackStore.temporal.getState().undo()}
          >
            <Undo2 className="size-3.5" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6"
            disabled={useMultiTrackStore.temporal.getState().futureStates.length === 0}
            onClick={() => useMultiTrackStore.temporal.getState().redo()}
          >
            <Redo2 className="size-3.5" />
          </Button>

          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6"
            onClick={() => { setPlayhead(0); pause(); }}
          >
            <SkipBack className="size-3.5" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6"
            onClick={togglePlay}
          >
            {isPlaying ? <Pause className="size-3.5" /> : <Play className="size-3.5" />}
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6"
            onClick={() => { setPlayhead(totalDuration); pause(); }}
          >
            <SkipForward className="size-3.5" />
          </Button>
        </div>

        <span className="font-mono text-muted-foreground ml-1">
          {formatTime(playhead)} / {formatTime(totalDuration)}
        </span>

        {selectedTransition && (
          <div className="flex items-center gap-2 ml-2 border-l pl-2 border-border">
            <span className="text-muted-foreground">转场时长</span>
            <Slider
              className="w-24"
              min={100}
              max={2000}
              step={50}
              value={[selectedTransition.duration_ms]}
              onValueChange={([v]) => {
                const params = JSON.parse(selectedTransition.params_json);
                params.duration_ms = v;
                updateItem(selectedTransition.id, {
                  position_ms: null,
                  duration_ms: v,
                  in_point_ms: null,
                  out_point_ms: v,
                  params_json: JSON.stringify(params),
                });
              }}
            />
            <span className="font-mono w-12 text-right">{selectedTransition.duration_ms}ms</span>
          </div>
        )}

        {selectedStickerItem && (() => {
          const sp = JSON.parse(selectedStickerItem.params_json) as Record<string, unknown>;
          const px = typeof sp.position_x === "number" ? sp.position_x : 0.5;
          const py = typeof sp.position_y === "number" ? sp.position_y : 0.5;
          const sc = typeof sp.scale === "number" ? sp.scale : 1.0;
          const updateSticker = (patch: Record<string, unknown>) => {
            updateItem(selectedStickerItem.id, {
              position_ms: null, duration_ms: null, in_point_ms: null, out_point_ms: null,
              params_json: JSON.stringify({ ...sp, ...patch }),
            });
          };
          return (
            <div className="flex items-center gap-2 ml-2 border-l pl-2 border-border">
              <span className="text-muted-foreground">X</span>
              <Slider className="w-16" min={0} max={1} step={0.01} value={[px]} onValueChange={([v]) => updateSticker({ position_x: v })} />
              <span className="text-muted-foreground">Y</span>
              <Slider className="w-16" min={0} max={1} step={0.01} value={[py]} onValueChange={([v]) => updateSticker({ position_y: v })} />
              <span className="text-muted-foreground">缩放</span>
              <Slider className="w-16" min={0.5} max={3} step={0.1} value={[sc]} onValueChange={([v]) => updateSticker({ scale: v })} />
              <span className="font-mono w-8 text-right">{sc.toFixed(1)}x</span>
            </div>
          );
        })()}

        <div className="flex items-center gap-2 ml-auto">
          <span className="text-muted-foreground">缩放</span>
          <Slider
            className="w-28"
            min={0.1}
            max={20}
            step={0.1}
            value={[zoom]}
            onValueChange={([v]) => setZoom(v)}
          />
          <span className="font-mono w-10 text-right">{zoom.toFixed(1)}x</span>
        </div>
      </div>

      <AssetPickerDialog
        open={pickerOpen}
        onOpenChange={setPickerOpen}
        projectId={projectId}
        projectRoot={projectRoot}
        assetType={pickerType}
        onSelect={handleAssetSelected}
      />

      <AddTextDialog
        open={textDialogOpen}
        onOpenChange={setTextDialogOpen}
        onSubmit={handleAddText}
      />

      <StickerPicker
        open={stickerPickerOpen}
        onOpenChange={setStickerPickerOpen}
        onSelect={(s) => addOverlayItem("sticker", 3000, JSON.stringify({ sticker_id: s.id, position_x: 0.5, position_y: 0.5, scale: 1.0 }), "贴纸")}
      />

      <ColorPresetPicker
        open={colorPresetPickerOpen}
        onOpenChange={setColorPresetPickerOpen}
        onSelect={(p) => addOverlayItem("effect", 5000, JSON.stringify({ effect_type: "color_preset", params: { preset_id: p.id } }), "调色")}
      />
    </>
  );
}
