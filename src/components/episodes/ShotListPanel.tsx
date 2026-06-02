import { useEffect, useMemo, useState } from "react";
import { Clapperboard, Plus } from "lucide-react";
import { toast } from "sonner";
import {
  DndContext,
  type DragEndEvent,
  KeyboardSensor,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import {
  SortableContext,
  arrayMove,
  sortableKeyboardCoordinates,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import {
  restrictToParentElement,
  restrictToVerticalAxis,
} from "@dnd-kit/modifiers";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { EmptyState } from "@/components/common/EmptyState";
import { ErrorAlert } from "@/components/common/ErrorAlert";
import { useReorderShots, useShotList } from "@/hooks/useShots";
import type { Shot } from "@/lib/bindings/commands";
import { ShotCard } from "./ShotCard";
import { CreateShotDialog } from "./CreateShotDialog";

interface Props {
  episodeId: string;
}

export function ShotListPanel({ episodeId }: Props) {
  const [createOpen, setCreateOpen] = useState(false);
  const shots = useShotList(episodeId);
  const reorder = useReorderShots(episodeId);

  const [localOrder, setLocalOrder] = useState<string[] | null>(null);

  useEffect(() => {
    setLocalOrder(null);
  }, [shots.data]);

  const orderedShots = useMemo<Shot[]>(() => {
    if (!shots.data) return [];
    if (!localOrder) return shots.data;
    const byId = new Map(shots.data.map((s) => [s.id, s]));
    const next: Shot[] = [];
    for (const id of localOrder) {
      const shot = byId.get(id);
      if (shot) next.push(shot);
    }
    return next.length === shots.data.length ? next : shots.data;
  }, [shots.data, localOrder]);

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  );

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) return;
    const currentIds = orderedShots.map((s) => s.id);
    const oldIndex = currentIds.indexOf(active.id as string);
    const newIndex = currentIds.indexOf(over.id as string);
    if (oldIndex < 0 || newIndex < 0) return;
    const nextIds = arrayMove(currentIds, oldIndex, newIndex);
    setLocalOrder(nextIds);
    reorder.mutate(nextIds, {
      onError: (err) => {
        setLocalOrder(null);
        toast.error(
          `分镜排序失败：${err instanceof Error ? err.message : String(err)}`,
        );
      },
    });
  };

  return (
    <section className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-medium text-muted-foreground">
          分镜列表 {shots.data && `· ${shots.data.length}`}
        </h2>
        <Button
          type="button"
          size="sm"
          variant="outline"
          onClick={() => setCreateOpen(true)}
        >
          <Plus className="mr-1 h-4 w-4" />
          新建分镜
        </Button>
      </div>
      {shots.isLoading && (
        <div className="space-y-2">
          <Skeleton className="h-16 w-full" />
          <Skeleton className="h-16 w-full" />
        </div>
      )}
      {shots.isError && (
        <ErrorAlert error={shots.error} onRetry={() => shots.refetch()} />
      )}
      {shots.data?.length === 0 && (
        <EmptyState
          icon={Clapperboard}
          title="尚无分镜"
          description="把剧本拆成一个个分镜，方便后续生成图像/视频。"
          action={
            <Button onClick={() => setCreateOpen(true)}>新建第一个分镜</Button>
          }
        />
      )}
      {orderedShots.length > 0 && (
        <DndContext
          sensors={sensors}
          collisionDetection={closestCenter}
          modifiers={[restrictToVerticalAxis, restrictToParentElement]}
          onDragEnd={handleDragEnd}
        >
          <SortableContext
            items={orderedShots.map((s) => s.id)}
            strategy={verticalListSortingStrategy}
          >
            <div className="flex flex-col gap-2">
              {orderedShots.map((shot, index) => (
                <ShotCard
                  key={shot.id}
                  episodeId={episodeId}
                  shot={shot}
                  displayIndex={index}
                />
              ))}
            </div>
          </SortableContext>
        </DndContext>
      )}
      <CreateShotDialog
        episodeId={episodeId}
        open={createOpen}
        onOpenChange={setCreateOpen}
      />
    </section>
  );
}
