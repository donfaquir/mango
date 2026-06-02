import { BaseEdge, getBezierPath, type EdgeProps } from "@xyflow/react";

export function ShotToTaskEdge({
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
        stroke: selected ? "#0ea5e9" : "#94a3b8",
        strokeWidth: 1.5,
        strokeDasharray: "6 4",
      }}
    />
  );
}
