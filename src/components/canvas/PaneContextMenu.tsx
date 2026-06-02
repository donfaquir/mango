import { useState } from "react";
import {
  FileText,
  Film,
  User,
  Cog,
  ImageIcon,
} from "lucide-react";
import { useParams } from "react-router-dom";
import type { XYPosition } from "@xyflow/react";
import { toast } from "sonner";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useCanvasStore } from "@/stores/canvasStore";
import {
  createAssetNode,
  createCharacterNode,
  createScriptNode,
  createStoryboardNode,
  createTaskNode,
} from "./nodeFactory";
import { ShotPickerDialog } from "./pickers/ShotPickerDialog";
import { CharacterPickerDialog } from "./pickers/CharacterPickerDialog";
import { TaskPickerDialog } from "./pickers/TaskPickerDialog";
import { AssetPickerDialog } from "@/components/assets/AssetPickerDialog";
import { useProject } from "@/hooks/useProjects";

export interface PaneMenuState {
  clientX: number;
  clientY: number;
  flow: XYPosition;
}

interface Props {
  state: PaneMenuState | null;
  onClose: () => void;
}

type PickerKind = "shot" | "character" | "task" | "asset" | null;

export function PaneContextMenu({ state, onClose }: Props) {
  const { projectId, episodeId } = useParams<{
    projectId: string;
    episodeId: string;
  }>();
  const { data: project } = useProject(projectId);
  const addNode = useCanvasStore((s) => s.addNode);
  const [picker, setPicker] = useState<PickerKind>(null);
  const [pickerPos, setPickerPos] = useState<XYPosition>({ x: 0, y: 0 });

  const closeAll = () => {
    setPicker(null);
    onClose();
  };

  const openPicker = (kind: Exclude<PickerKind, null>) => {
    if (!state) return;
    setPickerPos(state.flow);
    setPicker(kind);
    onClose();
  };

  return (
    <>
      {state && (
        <DropdownMenu open onOpenChange={(o) => !o && onClose()}>
          <DropdownMenuTrigger asChild>
            <button
              type="button"
              aria-hidden
              tabIndex={-1}
              style={{
                position: "fixed",
                left: state.clientX,
                top: state.clientY,
                width: 0,
                height: 0,
                opacity: 0,
                pointerEvents: "none",
              }}
            />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" sideOffset={2}>
            <DropdownMenuLabel>添加节点</DropdownMenuLabel>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              onSelect={() => {
                if (!episodeId) {
                  toast.error("缺少剧集上下文");
                  onClose();
                  return;
                }
                addNode(createScriptNode(episodeId, state.flow));
                onClose();
              }}
            >
              <FileText className="size-4" />
              剧本节点
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => openPicker("shot")}>
              <Film className="size-4" />
              分镜节点…
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => openPicker("character")}>
              <User className="size-4" />
              角色节点…
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => openPicker("task")}>
              <Cog className="size-4" />
              任务节点…
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => openPicker("asset")}>
              <ImageIcon className="size-4" />
              素材节点…
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      )}

      <ShotPickerDialog
        open={picker === "shot"}
        episodeId={episodeId}
        onOpenChange={(o) => !o && setPicker(null)}
        onSelect={(shotId) => {
          addNode(createStoryboardNode(shotId, pickerPos));
          closeAll();
        }}
      />

      <CharacterPickerDialog
        open={picker === "character"}
        projectId={projectId}
        onOpenChange={(o) => !o && setPicker(null)}
        onSelect={(characterId) => {
          addNode(createCharacterNode(characterId, pickerPos));
          closeAll();
        }}
      />

      <TaskPickerDialog
        open={picker === "task"}
        projectId={projectId}
        onOpenChange={(o) => !o && setPicker(null)}
        onSelect={(taskId) => {
          addNode(createTaskNode(taskId, pickerPos));
          closeAll();
        }}
      />

      {projectId && (
        <AssetPickerDialog
          open={picker === "asset"}
          projectId={projectId}
          projectRoot={project?.root_path}
          onOpenChange={(o) => !o && setPicker(null)}
          onSelect={(asset) => {
            addNode(createAssetNode(asset.id, pickerPos));
            closeAll();
          }}
        />
      )}
    </>
  );
}
