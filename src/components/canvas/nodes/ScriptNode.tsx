import type { NodeProps } from "@xyflow/react";
import { FileText } from "lucide-react";
import { useEpisode } from "@/hooks/useEpisodes";
import { useCanvasStore } from "@/stores/canvasStore";
import type { ScriptNode as ScriptNodeType } from "./types";
import { NodeShell } from "./shared/NodeShell";

const SCRIPT_PREVIEW_LINES = 3;

export function ScriptNode({
  id,
  data,
  selected,
}: NodeProps<ScriptNodeType>) {
  const { data: episode } = useEpisode(data.episodeId);
  const updateNodeData = useCanvasStore((s) => s.updateNodeData);

  const toggleCollapsed = () => {
    updateNodeData(id, { collapsed: !data.collapsed });
  };

  const previewLines = episode?.script_text
    ? episode.script_text.split("\n").slice(0, SCRIPT_PREVIEW_LINES)
    : [];

  return (
    <NodeShell selected={selected} onDoubleClick={toggleCollapsed} width={280}>
      <div className="flex items-center gap-2 border-b px-3 py-2 text-sm font-medium">
        <FileText className="size-4 text-muted-foreground" />
        <span className="truncate">{episode?.title ?? "剧本"}</span>
      </div>
      {!data.collapsed && (
        <div className="px-3 py-2 text-xs text-muted-foreground">
          {previewLines.length === 0 ? (
            <span className="italic">未填写剧本</span>
          ) : (
            previewLines.map((line, i) => (
              <p key={i} className="truncate">
                {line || " "}
              </p>
            ))
          )}
        </div>
      )}
    </NodeShell>
  );
}
