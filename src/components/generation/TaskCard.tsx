import { useState } from "react";
import { Button } from "@/components/ui/button";
import { TaskStatusBadge } from "./TaskStatusBadge";
import { TaskProgress } from "./TaskProgress";
import { ResultPreviewDialog } from "./ResultPreviewDialog";
import { useCancelTask, useRetryTask } from "@/hooks/useTasks";
import type { GenerationTask } from "@/lib/bindings/commands";

interface TaskCardProps {
  task: GenerationTask;
  projectId: string;
}

function formatRelative(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMin = Math.floor(diffMs / 60000);
  if (diffMin < 1) return "刚刚";
  if (diffMin < 60) return `${diffMin} 分钟前`;
  const diffHour = Math.floor(diffMin / 60);
  if (diffHour < 24) return `${diffHour} 小时前`;
  const diffDay = Math.floor(diffHour / 24);
  if (diffDay < 30) return `${diffDay} 天前`;
  return date.toLocaleDateString("zh-CN");
}

// Extract prompt from params_json
function getPrompt(paramsJson: string): string {
  try {
    const params = JSON.parse(paramsJson);
    return params.prompt ?? "";
  } catch {
    return "";
  }
}

// Extract progress from event payload stored in task
// The task status changed event carries progress, but the DB task row
// may not have it. We'll check if the external_task_id implies progress.
function getProgress(task: GenerationTask): number {
  // For now, running tasks without explicit progress show indeterminate
  // We could store progress in the task row later; for now return 50 for running
  if (task.status === "running") return 50;
  return 0;
}

export function TaskCard({ task, projectId }: TaskCardProps) {
  const cancel = useCancelTask();
  const retry = useRetryTask();
  const [previewOpen, setPreviewOpen] = useState(false);

  const isTerminal = ["success", "failed", "cancelled"].includes(task.status);
  const prompt = getPrompt(task.params_json);

  return (
    <div className="rounded-lg border bg-card p-3">
      <div className="flex gap-3">
        {/* Thumbnail placeholder or preview trigger */}
        <div className="h-20 w-20 shrink-0 overflow-hidden rounded-md bg-muted">
          {task.status === "success" && task.result_asset_id ? (
            <button
              type="button"
              className="flex h-full w-full items-center justify-center text-xs text-green-600"
              onClick={() => setPreviewOpen(true)}
            >
              点击预览
            </button>
          ) : (
            <div className="flex h-full w-full items-center justify-center text-xs text-muted-foreground">
              {task.task_type === "video" ? "视频" : "图片"}
            </div>
          )}
        </div>

        <div className="flex-1 min-w-0 space-y-1.5">
          <div className="flex items-center gap-2">
            <TaskStatusBadge status={task.status} />
            <span className="text-xs text-muted-foreground">
              {formatRelative(task.created_at)}
            </span>
          </div>
          {prompt && (
            <p className="line-clamp-2 text-sm">{prompt}</p>
          )}
          <p className="text-xs text-muted-foreground">
            {task.model_id} · {task.task_type}
          </p>
          {task.status === "running" && (
            <TaskProgress value={getProgress(task)} />
          )}
          {task.status === "failed" && task.error_message && (
            <p className="text-xs text-destructive line-clamp-2">
              {task.error_message}
            </p>
          )}

          {/* Actions */}
          <div className="flex gap-2 pt-1">
            {!isTerminal && (
              <Button
                size="sm"
                variant="outline"
                onClick={() => cancel.mutate(task.id)}
                disabled={cancel.isPending}
              >
                取消
              </Button>
            )}
            {task.status === "failed" && (
              <Button
                size="sm"
                variant="outline"
                onClick={() => retry.mutate(task.id)}
                disabled={retry.isPending}
              >
                重试
              </Button>
            )}
            {task.status === "success" && (
              <Button
                size="sm"
                variant="outline"
                onClick={() => setPreviewOpen(true)}
              >
                预览
              </Button>
            )}
          </div>
        </div>
      </div>

      {previewOpen && (
        <ResultPreviewDialog
          task={task}
          projectId={projectId}
          onClose={() => setPreviewOpen(false)}
        />
      )}
    </div>
  );
}
