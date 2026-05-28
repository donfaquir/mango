import { BaseEdge, getBezierPath, type EdgeProps } from "@xyflow/react";

export function TaskToAssetEdge({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  selected,
}: EdgeProps) {
  const [edgePath] = getBezierPath({
    sourceX,
    sourceY,
    targetX,
    targetY,
    sourcePosition,
    targetPosition,
  });
  return (
    <BaseEdge
      id={id}
      path={edgePath}
      style={{
        stroke: selected ? "#10b981" : "#94a3b8",
        strokeWidth: 1.5,
        strokeDasharray: "2 3",
      }}
    />
  );
}
