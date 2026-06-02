import { toast } from "sonner";
import { CheckIcon, GripVertical, Trash2 } from "lucide-react";
import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { Shot, ShotStatus } from "@/lib/bindings/commands";
import { useDeleteShot } from "@/hooks/useShots";

const STATUS_LABELS: Record<ShotStatus, string> = {
  draft: "草稿",
  ready: "就绪",
  generating: "生成中",
  done: "完成",
};

const STATUS_CLASSES: Record<ShotStatus, string> = {
  draft: "bg-muted text-muted-foreground",
  ready: "bg-blue-500/15 text-blue-700 dark:text-blue-300",
  generating: "bg-amber-500/15 text-amber-700 dark:text-amber-300",
  done: "bg-emerald-500/15 text-emerald-700 dark:text-emerald-300",
};

interface Props {
  episodeId: string;
  shot: Shot;
  displayIndex: number;
  selected?: boolean;
  onToggleSelect?: (next: boolean) => void;
}

export function ShotCard({
  episodeId,
  shot,
  displayIndex,
  selected = false,
  onToggleSelect,
}: Props) {
  const deleteShot = useDeleteShot(episodeId);
  const {
    attributes,
    listeners,
    setNodeRef,
    setActivatorNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: shot.id });

  const handleDelete = async () => {
    if (!window.confirm(`确认删除分镜 #${displayIndex + 1}？`)) return;
    try {
      await deleteShot.mutateAsync(shot.id);
      toast.success("分镜已删除");
    } catch (err) {
      toast.error(
        `删除失败：${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  return (
    <div
      ref={setNodeRef}
      style={{
        transform: CSS.Transform.toString(transform),
        transition,
      }}
      className={cn(
        "flex items-start gap-3 rounded-lg border bg-background p-3",
        isDragging && "z-10 opacity-50 shadow-lg",
        selected && "border-primary bg-primary/5",
      )}
    >
      {onToggleSelect && (
        <button
          type="button"
          role="checkbox"
          aria-checked={selected}
          aria-label={`选择分镜 #${displayIndex + 1}`}
          onClick={() => onToggleSelect(!selected)}
          className={cn(
            "mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-sm border",
            "transition hover:border-primary",
            selected
              ? "border-primary bg-primary text-primary-foreground"
              : "border-input",
          )}
        >
          {selected && <CheckIcon className="h-3 w-3" />}
        </button>
      )}
      <button
        ref={setActivatorNodeRef}
        type="button"
        aria-label="拖拽排序"
        className={cn(
          "shrink-0 self-stretch px-1 text-muted-foreground transition-colors",
          "cursor-grab touch-none hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring rounded",
          isDragging && "cursor-grabbing",
        )}
        {...attributes}
        {...listeners}
      >
        <GripVertical className="h-4 w-4" />
      </button>
      <div className="shrink-0 rounded-md bg-muted px-2 py-1 text-xs font-mono">
        #{displayIndex + 1}
      </div>
      <div className="min-w-0 flex-1 space-y-1">
        <div className="flex items-center gap-2">
          <span
            className={cn(
              "rounded px-1.5 py-0.5 text-xs",
              STATUS_CLASSES[shot.status],
            )}
          >
            {STATUS_LABELS[shot.status]}
          </span>
          {shot.duration_sec !== null && (
            <span className="text-xs text-muted-foreground">
              {shot.duration_sec}s
            </span>
          )}
        </div>
        <p className="text-sm">
          {shot.summary || (
            <span className="text-muted-foreground">（暂无摘要）</span>
          )}
        </p>
        {shot.dialogue && (
          <p className="text-xs text-muted-foreground">
            「{shot.dialogue}」
          </p>
        )}
      </div>
      <Button
        type="button"
        variant="ghost"
        size="icon"
        onClick={handleDelete}
        disabled={deleteShot.isPending}
        aria-label="删除分镜"
      >
        <Trash2 className="h-4 w-4" />
      </Button>
    </div>
  );
}
