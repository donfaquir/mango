import { Trash2 } from "lucide-react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useCanvasStore } from "@/stores/canvasStore";

export interface EdgeContextMenuState {
  edgeId: string;
  clientX: number;
  clientY: number;
}

interface Props {
  state: EdgeContextMenuState | null;
  onClose: () => void;
}

export function EdgeContextMenu({ state, onClose }: Props) {
  const removeEdge = useCanvasStore((s) => s.removeEdge);

  if (!state) return null;
  return (
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
        <DropdownMenuItem
          variant="destructive"
          onSelect={() => {
            removeEdge(state.edgeId);
            onClose();
          }}
        >
          <Trash2 className="size-4" />
          删除连线
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
