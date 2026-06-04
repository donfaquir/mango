import { type ComponentProps, type ElementType, useState } from "react";
import {
  useReactFlow,
  useStore as useFlowStore,
  useStoreApi as useFlowStoreApi,
} from "@xyflow/react";
import { useStore } from "zustand";
import {
  BookmarkPlus,
  History,
  Lock,
  Maximize,
  Redo2,
  Undo2,
  Unlock,
  ZoomIn,
  ZoomOut,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useCanvasStore } from "@/stores/canvasStore";
import { SaveVersionDialog } from "@/components/checkpoint/SaveVersionDialog";
import { VersionHistoryDialog } from "@/components/checkpoint/VersionHistoryDialog";
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
  const isInteractive = useFlowStore(
    (s) => s.nodesDraggable || s.nodesConnectable || s.elementsSelectable,
  );
  const flowStore = useFlowStoreApi();
  const [saveOpen, setSaveOpen] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);

  const onToggleLock = () => {
    const next = !isInteractive;
    flowStore.setState({
      nodesDraggable: next,
      nodesConnectable: next,
      elementsSelectable: next,
    });
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
        onClick={() => setSaveOpen(true)}
      />
      <ToolbarBtn
        icon={History}
        tooltip="版本历史"
        onClick={() => setHistoryOpen(true)}
      />
      <Separator orientation="vertical" className="mx-1 h-5" />
      <ToolbarBtn
        icon={isInteractive ? Unlock : Lock}
        tooltip={isInteractive ? "锁定画布" : "解锁画布"}
        onClick={onToggleLock}
      />
      <SaveVersionDialog
        open={saveOpen}
        onOpenChange={setSaveOpen}
        episodeId={episodeId}
        onBeforeSave={onBeforeSaveVersion}
      />
      <VersionHistoryDialog
        open={historyOpen}
        onOpenChange={setHistoryOpen}
        episodeId={episodeId}
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
