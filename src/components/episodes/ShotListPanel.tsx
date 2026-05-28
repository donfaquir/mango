import { useState } from "react";
import { Clapperboard, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { EmptyState } from "@/components/common/EmptyState";
import { ErrorAlert } from "@/components/common/ErrorAlert";
import { useShotList } from "@/hooks/useShots";
import { ShotCard } from "./ShotCard";
import { CreateShotDialog } from "./CreateShotDialog";

interface Props {
  episodeId: string;
}

export function ShotListPanel({ episodeId }: Props) {
  const [createOpen, setCreateOpen] = useState(false);
  const shots = useShotList(episodeId);

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
      {shots.data && shots.data.length > 0 && (
        <div className="flex flex-col gap-2">
          {shots.data.map((shot) => (
            <ShotCard key={shot.id} episodeId={episodeId} shot={shot} />
          ))}
        </div>
      )}
      <CreateShotDialog
        episodeId={episodeId}
        open={createOpen}
        onOpenChange={setCreateOpen}
      />
    </section>
  );
}
