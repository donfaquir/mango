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
      if (e.code === "Space") {
        e.preventDefault();
        useMultiTrackStore.getState().togglePlay();
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
