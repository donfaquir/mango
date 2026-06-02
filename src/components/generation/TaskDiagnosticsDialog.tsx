import { useMemo, useState } from "react";
import { AlertCircle, AlertTriangle, ChevronDown, ChevronRight, Info } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { TaskStatusBadge } from "./TaskStatusBadge";
import { useTaskEvents } from "@/hooks/useTasks";
import { cn } from "@/lib/utils";
import { parseDbDate } from "@/lib/datetime";
import type {
  GenerationTask,
  GenerationTaskEvent,
} from "@/lib/bindings/commands";

interface TaskDiagnosticsDialogProps {
  task: GenerationTask;
  onClose: () => void;
}

const PHASE_LABEL: Record<GenerationTaskEvent["phase"], string> = {
  submit_upload: "上传",
  submit_call: "提交",
  poll: "轮询",
  download: "下载",
  persist: "持久化",
  cleanup: "清理",
};

function formatTime(occurredAt: string): string {
  const d = parseDbDate(occurredAt);
  if (Number.isNaN(d.getTime())) return occurredAt;
  return d.toLocaleTimeString("zh-CN", { hour12: false });
}

function copy(text: string, ok: string) {
  void navigator.clipboard.writeText(text).then(
    () => toast.success(ok),
    () => toast.error("复制失败"),
  );
}

function buildReport(
  task: GenerationTask,
  events: GenerationTaskEvent[],
): string {
  const lines: string[] = [];
  lines.push(`# 诊断报告 - 任务 ${task.id}`);
  lines.push("");
  lines.push(`- 状态: ${task.status}`);
  lines.push(`- 模型: ${task.model_id}`);
  lines.push(`- 任务类型: ${task.task_type}`);
  if (task.external_task_id) {
    lines.push(`- 外部任务 ID: ${task.external_task_id}`);
  }
  lines.push(`- 创建时间: ${task.created_at}`);
  if (task.finished_at) {
    lines.push(`- 结束时间: ${task.finished_at}`);
  }
  if (task.error_message) {
    lines.push(`- 错误信息: ${task.error_message}`);
  }
  lines.push("");
  lines.push("## 事件时间线");
  if (events.length === 0) {
    lines.push("(无事件)");
  } else {
    for (const e of events) {
      const rid = e.request_id ? ` request_id=${e.request_id}` : "";
      const status = e.http_status != null ? ` HTTP ${e.http_status}` : "";
      lines.push(
        `- [${formatTime(e.occurred_at)}] ${PHASE_LABEL[e.phase]} (${e.severity})${status}${rid}: ${e.message}`,
      );
      if (e.details_json && e.details_json !== "{}") {
        lines.push(`  详情: ${e.details_json}`);
      }
    }
  }
  return lines.join("\n");
}

function EventRow({ event }: { event: GenerationTaskEvent }) {
  const [expanded, setExpanded] = useState(false);
  const hasDetails =
    event.request_id !== null ||
    event.http_status !== null ||
    (event.details_json && event.details_json !== "{}");
  const Icon =
    event.severity === "error"
      ? AlertCircle
      : event.severity === "warn"
        ? AlertTriangle
        : Info;
  const tone =
    event.severity === "error"
      ? "text-destructive"
      : event.severity === "warn"
        ? "text-amber-600 dark:text-amber-400"
        : "text-muted-foreground";
  return (
    <div className="border-b py-2 last:border-b-0">
      <button
        type="button"
        onClick={() => hasDetails && setExpanded((v) => !v)}
        className={cn(
          "flex w-full items-start gap-2 text-left",
          hasDetails && "cursor-pointer",
        )}
      >
        <Icon className={cn("mt-0.5 h-4 w-4 shrink-0", tone)} />
        <span className="font-mono text-xs text-muted-foreground">
          {formatTime(event.occurred_at)}
        </span>
        <span className="rounded bg-muted px-1.5 py-0.5 text-xs">
          {PHASE_LABEL[event.phase]}
        </span>
        <span className={cn("flex-1 text-sm", tone)}>{event.message}</span>
        {hasDetails &&
          (expanded ? (
            <ChevronDown className="mt-0.5 h-4 w-4 text-muted-foreground" />
          ) : (
            <ChevronRight className="mt-0.5 h-4 w-4 text-muted-foreground" />
          ))}
      </button>
      {expanded && hasDetails && (
        <div className="ml-6 mt-1 space-y-0.5 text-xs text-muted-foreground">
          {event.request_id && (
            <div>
              <span className="text-foreground">request_id:</span>{" "}
              <span className="font-mono">{event.request_id}</span>
            </div>
          )}
          {event.http_status != null && (
            <div>
              <span className="text-foreground">HTTP:</span> {event.http_status}
            </div>
          )}
          {event.details_json && event.details_json !== "{}" && (
            <pre className="overflow-x-auto rounded bg-muted p-2 text-xs">
              {event.details_json}
            </pre>
          )}
        </div>
      )}
    </div>
  );
}

export function TaskDiagnosticsDialog({ task, onClose }: TaskDiagnosticsDialogProps) {
  const eventsQuery = useTaskEvents(task.id);
  const events = eventsQuery.data ?? [];

  const latestRequestId = useMemo(() => {
    for (let i = events.length - 1; i >= 0; i--) {
      if (events[i].request_id) return events[i].request_id;
    }
    return null;
  }, [events]);

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <span>任务诊断</span>
            <TaskStatusBadge status={task.status} />
          </DialogTitle>
          <div className="space-y-0.5 pt-1 text-xs text-muted-foreground">
            <div>
              <span className="font-mono">{task.id.slice(0, 8)}</span> ·{" "}
              {task.model_id} · {task.task_type}
            </div>
            {task.external_task_id && (
              <div>
                外部任务 ID:{" "}
                <span className="font-mono">{task.external_task_id}</span>
              </div>
            )}
            <div>创建时间: {task.created_at}</div>
          </div>
        </DialogHeader>

        <div className="max-h-[60vh] overflow-y-auto rounded-md border bg-background p-3">
          {eventsQuery.isLoading ? (
            <p className="text-center text-sm text-muted-foreground">加载中…</p>
          ) : events.length === 0 ? (
            <p className="text-center text-sm text-muted-foreground">
              （暂无诊断事件）
            </p>
          ) : (
            events.map((e) => <EventRow key={e.id} event={e} />)
          )}
        </div>

        <div className="flex justify-end gap-2">
          <Button
            size="sm"
            variant="outline"
            onClick={() =>
              latestRequestId
                ? copy(latestRequestId, "request_id 已复制")
                : toast.info("当前任务暂无 request_id")
            }
          >
            复制 request_id
          </Button>
          <Button
            size="sm"
            variant="outline"
            onClick={() => copy(buildReport(task, events), "诊断报告已复制")}
          >
            复制诊断报告
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
