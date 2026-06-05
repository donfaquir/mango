import { useCallback } from "react";
import {
  DndContext,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext,
  sortableKeyboardCoordinates,
  verticalListSortingStrategy,
  useSortable,
  arrayMove,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ClipCard } from "./ClipCard";
import {
  useVideoClipList,
  useDeleteVideoClip,
  useReorderVideoClips,
} from "@/hooks/useVideoClips";
import type { VideoClip } from "@/lib/bindings/commands";
import { cn } from "@/lib/utils";

interface ClipAssemblyProps {
  episodeId: string;
  onAddClip: () => void;
  onExport: () => void;
  className?: string;
}

function SortableClip({
  clip,
  onDelete,
}: {
  clip: VideoClip;
  onDelete: (id: string) => void;
}) {
  const { attributes, listeners, setNodeRef, transform, transition } =
    useSortable({ id: clip.id });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  return (
    <div ref={setNodeRef} style={style} {...attributes}>
      <ClipCard clip={clip} onDelete={onDelete} dragHandleProps={listeners} />
    </div>
  );
}

export function ClipAssembly({
  episodeId,
  onAddClip,
  onExport,
  className,
}: ClipAssemblyProps) {
  const { data: clips = [] } = useVideoClipList(episodeId);
  const deleteClip = useDeleteVideoClip(episodeId);
  const reorderClips = useReorderVideoClips(episodeId);

  const sensors = useSensors(
    useSensor(PointerSensor),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  const handleDragEnd = useCallback(
    (event: DragEndEvent) => {
      const { active, over } = event;
      if (!over || active.id === over.id) return;
      const oldIndex = clips.findIndex((c) => c.id === active.id);
      const newIndex = clips.findIndex((c) => c.id === over.id);
      const reordered = arrayMove(clips, oldIndex, newIndex);
      reorderClips.mutate(reordered.map((c) => c.id));
    },
    [clips, reorderClips],
  );

  const handleDelete = useCallback(
    (id: string) => {
      if (window.confirm("确认删除此片段？")) {
        deleteClip.mutate(id);
      }
    },
    [deleteClip],
  );

  return (
    <div className={cn("flex flex-col gap-1.5 p-2", className)}>
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium text-muted-foreground">
          片段列表 ({clips.length})
        </span>
        <div className="flex gap-1">
          <Button variant="ghost" size="sm" className="h-7 text-xs" onClick={onAddClip}>
            <Plus className="h-3 w-3 mr-1" />
            添加
          </Button>
          <Button
            variant="default"
            size="sm"
            className="h-7 text-xs"
            disabled={clips.length === 0}
            onClick={onExport}
          >
            导出
          </Button>
        </div>
      </div>

      {clips.length === 0 ? (
        <div className="text-xs text-muted-foreground text-center py-4">
          暂无片段，从时间轴添加裁剪片段或点击「添加」按钮
        </div>
      ) : (
        <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
          <SortableContext items={clips.map((c) => c.id)} strategy={verticalListSortingStrategy}>
            <div className="flex flex-col gap-1">
              {clips.map((clip) => (
                <SortableClip key={clip.id} clip={clip} onDelete={handleDelete} />
              ))}
            </div>
          </SortableContext>
        </DndContext>
      )}
    </div>
  );
}
