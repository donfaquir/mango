import { useEffect } from "react";
import { toast } from "sonner";
import { useMultiTrackStore } from "@/stores/multiTrackStore";
import { usePlaybackLoop } from "@/hooks/usePlaybackLoop";
import { VideoPreview } from "./VideoPreview";
import { MultiTrackTimeline } from "./MultiTrackTimeline";
import { MultiTrackControls } from "./MultiTrackControls";
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
    <div className={cn("flex flex-col gap-0", className)}>
      <VideoPreview projectRoot={projectRoot} className="h-80" />
      <MultiTrackTimeline episodeId={episodeId} projectRoot={projectRoot} />
      <MultiTrackControls episodeId={episodeId} projectId={projectId} projectRoot={projectRoot} />
    </div>
  );
}
