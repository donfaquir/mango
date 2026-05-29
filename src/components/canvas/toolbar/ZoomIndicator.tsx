import { useStore as useReactFlowInternalStore } from "@xyflow/react";

export function ZoomIndicator() {
  const zoom = useReactFlowInternalStore((s) => s.transform[2]);
  return (
    <span className="inline-block w-12 text-center text-xs tabular-nums text-muted-foreground">
      {Math.round(zoom * 100)}%
    </span>
  );
}
