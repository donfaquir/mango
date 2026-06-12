import { useState, useCallback } from "react";
import { join } from "@tauri-apps/api/path";
import { Download, Plus, Video, Music } from "lucide-react";
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
import { useMultiTrackStore } from "@/stores/multiTrackStore";
import { commands, type Asset } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

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
  const { playhead, zoom, setZoom, totalDuration, importing, items, tracks, addItem, importFromShots } =
    useMultiTrackStore();
  const [pickerOpen, setPickerOpen] = useState(false);
  const [pickerType, setPickerType] = useState<"video" | "audio">("video");

  const itemCount = Object.keys(items).length;

  const handleImportFromShots = useCallback(async () => {
    if (itemCount > 0) {
      const ok = window.confirm(`Timeline already has ${itemCount} items. Import will add clips without removing existing items. Continue?`);
      if (!ok) return;
    }
    try {
      await importFromShots(episodeId);
      toast.success("Import from shots completed");
    } catch (err) {
      toast.error(`Import failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  }, [episodeId, itemCount, importFromShots]);

  const openPicker = useCallback((type: "video" | "audio") => {
    setPickerType(type);
    setPickerOpen(true);
  }, []);

  const handleAssetSelected = useCallback(async (asset: Asset) => {
    const targetTrack = tracks.find((t) =>
      pickerType === "video" ? t.track_type === "video" : t.track_type === "audio",
    );
    if (!targetTrack) {
      toast.error("No matching track found");
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
      toast.success("Media added to timeline");
    } catch (err) {
      toast.error(`Failed to add media: ${err instanceof Error ? err.message : String(err)}`);
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
          {importing ? "Importing..." : "Import Shots"}
        </Button>

        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="outline" size="sm" className="h-6 text-xs">
              <Plus className="size-3.5" />
              Add Media
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent>
            <DropdownMenuItem onClick={() => openPicker("video")}>
              <Video className="size-4" />
              Add Video
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => openPicker("audio")}>
              <Music className="size-4" />
              Add Audio
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>

        <span className="font-mono text-muted-foreground ml-2">
          {formatTime(playhead)} / {formatTime(totalDuration)}
        </span>

        <div className="flex items-center gap-2 ml-auto">
          <span className="text-muted-foreground">Zoom</span>
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
    </>
  );
}
