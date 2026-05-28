import { Handle, Position } from "@xyflow/react";
import type { ReactNode } from "react";
import type { ShotStatus, GenerationTaskStatus } from "@/lib/bindings/commands";
import { cn } from "@/lib/utils";

export type StatusKind = ShotStatus | GenerationTaskStatus;

// Explicit Tailwind class lookup — Tailwind v4 cannot statically extract
// `border-${x}-300` patterns, so dynamic class names are forbidden.
export const STATUS_BORDER: Record<string, string> = {
  draft: "border-zinc-300",
  ready: "border-blue-400",
  generating: "border-amber-400",
  done: "border-emerald-500",
  pending: "border-zinc-300",
  running: "border-amber-400",
  success: "border-emerald-500",
  failed: "border-red-500",
  cancelled: "border-zinc-400",
};

interface NodeShellProps {
  selected?: boolean;
  status?: StatusKind;
  onDoubleClick?: () => void;
  children: ReactNode;
  width?: number;
}

export function NodeShell({
  selected,
  status,
  onDoubleClick,
  children,
  width = 240,
}: NodeShellProps) {
  const statusBorder = status ? STATUS_BORDER[status] : "border-border";
  return (
    <div
      onDoubleClick={onDoubleClick}
      style={{ width }}
      className={cn(
        "rounded-md border-2 bg-card text-card-foreground shadow-sm transition-shadow",
        statusBorder,
        selected && "ring-2 ring-primary/40",
      )}
    >
      <Handle type="target" position={Position.Left} />
      {children}
      <Handle type="source" position={Position.Right} />
    </div>
  );
}
