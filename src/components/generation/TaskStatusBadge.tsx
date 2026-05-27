import { Loader2 } from "lucide-react";
import { cn } from "@/lib/utils";
import type { GenerationTaskStatus } from "@/lib/bindings/commands";

const STYLE: Record<GenerationTaskStatus, { label: string; className: string }> = {
  pending: {
    label: "排队中",
    className: "bg-muted text-muted-foreground",
  },
  running: {
    label: "生成中",
    className: "bg-blue-100 text-blue-900 dark:bg-blue-950 dark:text-blue-200",
  },
  success: {
    label: "成功",
    className: "bg-green-100 text-green-900 dark:bg-green-950 dark:text-green-200",
  },
  failed: {
    label: "失败",
    className: "bg-red-100 text-red-900 dark:bg-red-950 dark:text-red-200",
  },
  cancelled: {
    label: "已取消",
    className: "bg-muted text-muted-foreground line-through",
  },
};

export function TaskStatusBadge({ status }: { status: GenerationTaskStatus }) {
  const s = STYLE[status];
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium",
        s.className,
      )}
    >
      {status === "running" && (
        <Loader2 className="mr-1 h-3 w-3 animate-spin" />
      )}
      {s.label}
    </span>
  );
}
