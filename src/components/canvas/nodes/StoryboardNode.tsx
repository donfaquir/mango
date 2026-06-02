import type { NodeProps } from "@xyflow/react";
import { Film, X } from "lucide-react";
import { useShot } from "@/hooks/useShots";
import { useCanvasStore } from "@/stores/canvasStore";
import { Button } from "@/components/ui/button";
import type { StoryboardNode as StoryboardNodeType } from "./types";
import { NodeShell } from "./shared/NodeShell";

export function StoryboardNode({
  id,
  data,
  selected,
}: NodeProps<StoryboardNodeType>) {
  const { data: shot, isLoading } = useShot(data.shotId);
  const removeNode = useCanvasStore((s) => s.removeNode);

  if (!isLoading && !shot) {
    return (
      <NodeShell selected={selected} width={240}>
        <div className="flex items-center justify-between border-b border-red-300 px-3 py-2 text-sm font-medium text-red-600">
          <span>分镜已删除</span>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="size-6"
            onClick={() => removeNode(id)}
          >
            <X className="size-3" />
          </Button>
        </div>
        <div className="px-3 py-2 text-xs text-muted-foreground">
          shot_id: {data.shotId.slice(0, 8)}…
        </div>
      </NodeShell>
    );
  }

  return (
    <NodeShell selected={selected} status={shot?.status} width={260}>
      <div className="flex items-center gap-2 border-b px-3 py-2 text-sm font-medium">
        <Film className="size-4 text-muted-foreground" />
        <span className="truncate">分镜 #{shot?.order_index ?? "—"}</span>
      </div>
      <div className="space-y-1 px-3 py-2 text-xs">
        <p className="line-clamp-2 text-muted-foreground">
          {shot?.summary || <span className="italic">无摘要</span>}
        </p>
        {shot?.dialogue && (
          <p className="line-clamp-1 italic">「{shot.dialogue}」</p>
        )}
        <p className="text-[11px] text-muted-foreground">
          {shot?.duration_sec ? `${shot.duration_sec}s` : "时长未设"}
        </p>
      </div>
    </NodeShell>
  );
}
