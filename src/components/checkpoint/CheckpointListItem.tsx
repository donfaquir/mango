import { Bookmark, Clock, MoreHorizontal, RotateCcw, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { EpisodeCheckpointListItem as CheckpointItem } from "@/lib/bindings/commands";
import { formatRelative } from "@/lib/datetime";

interface Props {
  item: CheckpointItem;
  onRestore: (item: CheckpointItem) => void;
  onDelete: (item: CheckpointItem) => void;
}

export function CheckpointListItem({ item, onRestore, onDelete }: Props) {
  const isManual = item.trigger_type === "manual";
  const Icon = isManual ? Bookmark : Clock;

  const title = isManual
    ? `版本 ${item.version_number}${item.label ? ` · ${item.label}` : ""}`
    : "自动备份";

  const subtitle = item.change_summary
    ?? (isManual ? undefined : "定时保存");

  return (
    <div className="flex items-start gap-3 rounded-md border px-3 py-2.5">
      <Icon className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
      <div className="min-w-0 flex-1 space-y-0.5">
        <p className="text-sm font-medium leading-tight">{title}</p>
        {subtitle && (
          <p className="text-xs text-muted-foreground line-clamp-2">
            {subtitle}
          </p>
        )}
        <p className="text-xs text-muted-foreground">
          {formatRelative(item.created_at)}
        </p>
      </div>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" size="icon" className="h-7 w-7 shrink-0">
            <MoreHorizontal className="size-4" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem onClick={() => onRestore(item)}>
            <RotateCcw className="mr-2 size-4" />
            恢复到此版本
          </DropdownMenuItem>
          <DropdownMenuItem
            variant="destructive"
            onClick={() => onDelete(item)}
          >
            <Trash2 className="mr-2 size-4" />
            删除
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}
