import { type ReactNode } from "react";
import { Trash2, Scissors, Copy, ArrowRightLeft } from "lucide-react";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuShortcut,
  ContextMenuSub,
  ContextMenuSubContent,
  ContextMenuSubTrigger,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { useMultiTrackStore } from "@/stores/multiTrackStore";
import { useTransitionPresets } from "@/hooks/useKeyframes";

interface ItemContextMenuProps {
  itemId: string;
  itemType: string;
  trackType: string;
  trackLocked: boolean;
  children: ReactNode;
}

export function ItemContextMenu({ itemId, itemType, trackType, trackLocked, children }: ItemContextMenuProps) {
  const { removeItem, splitItem, duplicateItem, addTransition, playhead } = useMultiTrackStore();
  const { data: presets } = useTransitionPresets();
  const showTransition = itemType === "clip" && trackType === "video";

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent className="w-44">
        <ContextMenuItem
          disabled={trackLocked}
          onClick={() => removeItem(itemId)}
        >
          <Trash2 className="size-4" />
          删除
          <ContextMenuShortcut>Del</ContextMenuShortcut>
        </ContextMenuItem>
        <ContextMenuItem
          disabled={trackLocked}
          onClick={() => splitItem(itemId, playhead)}
        >
          <Scissors className="size-4" />
          分割
          <ContextMenuShortcut>S</ContextMenuShortcut>
        </ContextMenuItem>
        <ContextMenuItem
          disabled={trackLocked}
          onClick={() => duplicateItem(itemId)}
        >
          <Copy className="size-4" />
          复制
          <ContextMenuShortcut>⌘D</ContextMenuShortcut>
        </ContextMenuItem>
        {showTransition && presets && (
          <ContextMenuSub>
            <ContextMenuSubTrigger disabled={trackLocked}>
              <ArrowRightLeft className="size-4" />
              添加转场
            </ContextMenuSubTrigger>
            <ContextMenuSubContent className="w-36">
              {presets.map((p) => (
                <ContextMenuItem
                  key={p.id}
                  onClick={() => addTransition(itemId, p.id, 500)}
                >
                  {p.label}
                </ContextMenuItem>
              ))}
            </ContextMenuSubContent>
          </ContextMenuSub>
        )}
      </ContextMenuContent>
    </ContextMenu>
  );
}
