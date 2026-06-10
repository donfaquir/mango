import { useEffect } from "react";
import { toast } from "sonner";
import { useMultiTrackStore } from "@/stores/multiTrackStore";
import { MultiTrackTimeline } from "./MultiTrackTimeline";
import { MultiTrackControls } from "./MultiTrackControls";
import { cn } from "@/lib/utils";

interface MultiTrackEditorProps {
  episodeId: string;
  className?: string;
}

export function MultiTrackEditor({ episodeId, className }: MultiTrackEditorProps) {
  const reset = useMultiTrackStore((s) => s.reset);
  const init = useMultiTrackStore((s) => s.init);

  useEffect(() => {
    init(episodeId).catch((e) => {
      toast.error(`Timeline load failed: ${e instanceof Error ? e.message : String(e)}`);
    });
    return () => {
      reset();
    };
  }, [episodeId, init, reset]);

  return (
    <div className={cn("flex flex-col gap-0", className)}>
      <MultiTrackTimeline />
      <MultiTrackControls />
    </div>
  );
}
