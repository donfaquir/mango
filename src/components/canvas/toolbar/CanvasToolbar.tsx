import { type ComponentProps, type ElementType } from "react";
import { useReactFlow } from "@xyflow/react";
import { useStore } from "zustand";
import {
  BookmarkPlus,
  Maximize,
  Redo2,
  Undo2,
  ZoomIn,
  ZoomOut,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useCanvasStore } from "@/stores/canvasStore";
import { useCreateCheckpoint } from "@/hooks/useCheckpoint";
import { ZoomIndicator } from "./ZoomIndicator";

interface CanvasToolbarProps {
  episodeId: string;
  /**
   * Caller hook to flush pending autoSave debounce *before* checkpoint
   * creation, so the snapshot read on the backend reflects the latest UI.
   */
  onBeforeSaveVersion?: () => Promise<void> | void;
}

export function CanvasToolbar({
  episodeId,
  onBeforeSaveVersion,
}: CanvasToolbarProps) {
  const { zoomIn, zoomOut, fitView } = useReactFlow();
  const pastLen = useStore(
    useCanvasStore.temporal,
    (s) => s.pastStates.length,
  );
  const futureLen = useStore(
    useCanvasStore.temporal,
    (s) => s.futureStates.length,
  );
  const createCheckpoint = useCreateCheckpoint();

  const onSaveVersion = async () => {
    try {
      await onBeforeSaveVersion?.();
      await createCheckpoint.mutateAsync({ episode_id: episodeId, label: null });
      toast.success("已保存版本");
    } catch (e) {
      toast.error(`保存版本失败：${String(e)}`);
    }
  };

  return (
    <div className="absolute left-4 top-4 z-10 flex items-center gap-1 rounded-md border bg-background p-1 shadow">
      <ToolbarBtn
        icon={ZoomOut}
        tooltip="缩小"
        onClick={() => zoomOut({ duration: 200 })}
      />
      <ZoomIndicator />
      <ToolbarBtn
        icon={ZoomIn}
        tooltip="放大"
        onClick={() => zoomIn({ duration: 200 })}
      />
      <ToolbarBtn
        icon={Maximize}
        tooltip="适应视图"
        onClick={() => fitView({ padding: 0.2, duration: 300 })}
      />
      <Separator orientation="vertical" className="mx-1 h-5" />
      <ToolbarBtn
        icon={Undo2}
        tooltip="撤销 (Cmd+Z)"
        disabled={pastLen === 0}
        onClick={() => useCanvasStore.temporal.getState().undo()}
      />
      <ToolbarBtn
        icon={Redo2}
        tooltip="重做 (Cmd+Shift+Z)"
        disabled={futureLen === 0}
        onClick={() => useCanvasStore.temporal.getState().redo()}
      />
      <Separator orientation="vertical" className="mx-1 h-5" />
      <ToolbarBtn
        icon={BookmarkPlus}
        tooltip="保存版本"
        disabled={createCheckpoint.isPending}
        onClick={onSaveVersion}
      />
    </div>
  );
}

interface ToolbarBtnProps
  extends Omit<ComponentProps<typeof Button>, "children"> {
  icon: ElementType;
  tooltip: string;
}

function ToolbarBtn({ icon: Icon, tooltip, ...props }: ToolbarBtnProps) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          size="icon"
          variant="ghost"
          className="h-7 w-7"
          aria-label={tooltip}
          {...props}
        >
          <Icon className="h-4 w-4" />
        </Button>
      </TooltipTrigger>
      <TooltipContent side="bottom">{tooltip}</TooltipContent>
    </Tooltip>
  );
}
