import { useEffect, useMemo } from "react";
import { toast } from "sonner";
import { useMultiTrackStore } from "@/stores/multiTrackStore";
import { usePlaybackLoop } from "@/hooks/usePlaybackLoop";
import { VideoPreview } from "./VideoPreview";
import { MultiTrackTimeline } from "./MultiTrackTimeline";
import { MultiTrackControls } from "./MultiTrackControls";
import { KenBurnsSelector } from "./KenBurnsSelector";
import { TextEditorPanel } from "./TextEditorPanel";
import { cn } from "@/lib/utils";

interface MultiTrackEditorProps {
  episodeId: string;
  projectId: string;
  projectRoot: string | undefined;
  className?: string;
}

export function MultiTrackEditor({ episodeId, projectId, projectRoot, className }: MultiTrackEditorProps) {
  const reset = useMultiTrackStore((s) => s.reset);
  const init = useMultiTrackStore((s) => s.init);

  useEffect(() => {
    init(episodeId).catch((e) => {
      toast.error(`时间轴加载失败：${e instanceof Error ? e.message : String(e)}`);
    });
    return () => {
      reset();
    };
  }, [episodeId, init, reset]);

  usePlaybackLoop();

  const selection = useMultiTrackStore((s) => s.selection);
  const items = useMultiTrackStore((s) => s.items);
  const tracks = useMultiTrackStore((s) => s.tracks);

  const selectedClipIds = useMemo(() => {
    const videoTrackIds = new Set(tracks.filter((t) => t.track_type === "video").map((t) => t.id));
    return [...selection].filter((id) => {
      const item = items[id];
      return item && item.item_type === "clip" && videoTrackIds.has(item.track_id);
    });
  }, [selection, items, tracks]);

  const selectedTextItemId = useMemo(() => {
    if (selection.size !== 1) return null;
    const id = [...selection][0];
    const item = items[id];
    return item?.item_type === "text" ? id : null;
  }, [selection, items]);

  const currentPresetId = useMemo(() => {
    if (selectedClipIds.length !== 1) return null;
    const item = items[selectedClipIds[0]];
    if (!item) return null;
    try {
      const params = JSON.parse(item.params_json) as Record<string, unknown>;
      return typeof params.ken_burns_preset === "string" ? params.ken_burns_preset : null;
    } catch {
      return null;
    }
  }, [selectedClipIds, items]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement).tagName;
      if (tag === "INPUT" || tag === "TEXTAREA") return;

      const store = useMultiTrackStore.getState();
      const mod = e.metaKey || e.ctrlKey;

      if (e.code === "Space") {
        e.preventDefault();
        store.togglePlay();
        return;
      }

      if (e.code === "Delete" || e.code === "Backspace") {
        if (store.selection.size === 0) return;
        e.preventDefault();
        const lockedTrackIds = new Set(store.tracks.filter((t) => t.locked).map((t) => t.id));
        for (const id of store.selection) {
          const item = store.items[id];
          if (item && !lockedTrackIds.has(item.track_id)) {
            store.removeItem(id);
          }
        }
        return;
      }

      if (e.code === "KeyS" && !mod) {
        if (store.selection.size === 0) return;
        e.preventDefault();
        for (const id of store.selection) {
          store.splitItem(id, store.playhead);
        }
        return;
      }

      if (e.code === "KeyD" && mod && !e.shiftKey) {
        e.preventDefault();
        const first = store.selection.values().next().value;
        if (first) store.duplicateItem(first);
        return;
      }

      if (e.code === "KeyZ" && mod) {
        e.preventDefault();
        const temporal = useMultiTrackStore.temporal.getState();
        if (e.shiftKey) {
          temporal.redo();
        } else {
          temporal.undo();
        }
        return;
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, []);

  return (
    <div className={cn("flex flex-row", className)}>
      <div className="flex flex-col gap-0 flex-1 min-w-0">
        <VideoPreview projectRoot={projectRoot} className="h-80" />
        <MultiTrackTimeline episodeId={episodeId} projectRoot={projectRoot} />
        <MultiTrackControls episodeId={episodeId} projectId={projectId} projectRoot={projectRoot} />
        {selectedClipIds.length > 0 && (
          <KenBurnsSelector selectedClipIds={selectedClipIds} currentPresetId={currentPresetId} />
        )}
      </div>
      {selectedTextItemId && <TextEditorPanel itemId={selectedTextItemId} />}
    </div>
  );
}
