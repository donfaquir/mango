import { useNavigate, useParams } from "react-router-dom";
import { Trash2, ExternalLink } from "lucide-react";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { useCanvasStore } from "@/stores/canvasStore";
import type { CanvasNodeData } from "./types";

interface Props {
  nodeId: string;
  data: CanvasNodeData;
  children: React.ReactNode;
}

function detailHrefFor(
  data: CanvasNodeData,
  projectId: string | undefined,
  episodeId: string | undefined,
): string | null {
  if (!projectId) return null;
  switch (data.kind) {
    case "script":
      return `/project/${projectId}/episodes/${data.episodeId}`;
    case "storyboard":
      return episodeId
        ? `/project/${projectId}/episodes/${episodeId}#shot-${data.shotId}`
        : null;
    case "character":
      return `/project/${projectId}/subjects/character`;
    case "task":
      return `/project/${projectId}/generation`;
    case "asset":
      return `/project/${projectId}/assets`;
  }
}

export function NodeContextMenu({ nodeId, data, children }: Props) {
  const navigate = useNavigate();
  const { projectId, episodeId } = useParams<{
    projectId: string;
    episodeId: string;
  }>();
  const removeNode = useCanvasStore((s) => s.removeNode);

  const href = detailHrefFor(data, projectId, episodeId);

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem
          disabled={!href}
          onSelect={() => href && navigate(href)}
        >
          <ExternalLink className="size-4" />
          跳转详情
        </ContextMenuItem>
        <ContextMenuItem variant="destructive" onSelect={() => removeNode(nodeId)}>
          <Trash2 className="size-4" />
          从画布移除
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}
