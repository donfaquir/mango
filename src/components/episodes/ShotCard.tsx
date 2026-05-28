import { toast } from "sonner";
import { Trash2 } from "lucide-react";
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
}

export function ShotCard({ episodeId, shot }: Props) {
  const deleteShot = useDeleteShot(episodeId);

  const handleDelete = async () => {
    if (!window.confirm(`确认删除分镜 #${shot.order_index + 1}？`)) return;
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
    <div className="flex items-start gap-3 rounded-lg border p-3">
      <div className="shrink-0 rounded-md bg-muted px-2 py-1 text-xs font-mono">
        #{shot.order_index + 1}
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
