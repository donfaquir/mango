import type { NodeProps } from "@xyflow/react";
import { useState } from "react";
import { AlertCircle, Cog } from "lucide-react";
import { useTaskById } from "@/hooks/useTasks";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogTitle,
} from "@/components/ui/dialog";
import type { TaskNode as TaskNodeType } from "./types";
import { NodeShell } from "./shared/NodeShell";

export function TaskNode({ data, selected }: NodeProps<TaskNodeType>) {
  const { data: task } = useTaskById(data.taskId);
  const [diagOpen, setDiagOpen] = useState(false);

  return (
    <NodeShell selected={selected} status={task?.status} width={240}>
      <div className="flex items-center gap-2 border-b px-3 py-2 text-sm font-medium">
        <Cog className="size-4 text-muted-foreground" />
        <span className="truncate capitalize">
          {task?.task_type ?? "任务"}
        </span>
        <span className="ml-auto text-[11px] text-muted-foreground">
          {task?.status ?? "—"}
        </span>
      </div>
      <div className="space-y-1 px-3 py-2 text-xs text-muted-foreground">
        <p className="truncate">
          {task?.provider_id ?? "—"} / {task?.model_id ?? "—"}
        </p>
        {task?.status === "failed" && (
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-6 w-full justify-start gap-1 text-xs text-red-600"
            onClick={() => setDiagOpen(true)}
          >
            <AlertCircle className="size-3" />
            查看诊断
          </Button>
        )}
      </div>
      <Dialog open={diagOpen} onOpenChange={setDiagOpen}>
        <DialogContent>
          <DialogTitle>任务诊断</DialogTitle>
          <pre className="max-h-[60vh] overflow-auto rounded bg-muted p-3 text-xs">
            {task?.error_message ?? "（无错误信息）"}
          </pre>
        </DialogContent>
      </Dialog>
    </NodeShell>
  );
}
