import { type ReactNode } from "react";
import { Trash2, Scissors, Copy } from "lucide-react";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuShortcut,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { useMultiTrackStore } from "@/stores/multiTrackStore";

interface ItemContextMenuProps {
  itemId: string;
  trackLocked: boolean;
  children: ReactNode;
}

export function ItemContextMenu({ itemId, trackLocked, children }: ItemContextMenuProps) {
  const { removeItem, splitItem, duplicateItem, playhead } = useMultiTrackStore();

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent className="w-44">
        <ContextMenuItem
          disabled={trackLocked}
          onClick={() => removeItem(itemId)}
        >
          <Trash2 className="size-4" />
          Delete
          <ContextMenuShortcut>Del</ContextMenuShortcut>
        </ContextMenuItem>
        <ContextMenuItem
          disabled={trackLocked}
          onClick={() => splitItem(itemId, playhead)}
        >
          <Scissors className="size-4" />
          Split
          <ContextMenuShortcut>S</ContextMenuShortcut>
        </ContextMenuItem>
        <ContextMenuItem
          disabled={trackLocked}
          onClick={() => duplicateItem(itemId)}
        >
          <Copy className="size-4" />
          Duplicate
          <ContextMenuShortcut>⌘D</ContextMenuShortcut>
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}
