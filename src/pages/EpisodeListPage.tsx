import { useState } from "react";
import { Navigate, useParams } from "react-router-dom";
import { BookOpen, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { EmptyState } from "@/components/common/EmptyState";
import { ErrorAlert } from "@/components/common/ErrorAlert";
import { useEpisodeList } from "@/hooks/useEpisodes";
import { EpisodeCard } from "@/components/episodes/EpisodeCard";
import { CreateEpisodeDialog } from "@/components/episodes/CreateEpisodeDialog";

export default function EpisodeListPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const [createOpen, setCreateOpen] = useState(false);
  const episodes = useEpisodeList(projectId);

  if (!projectId) return <Navigate to="/" replace />;

  return (
    <div className="flex flex-col gap-4 p-1">
      <header className="flex items-center justify-between">
        <h2 className="text-xl font-semibold">剧本与分镜</h2>
        <Button onClick={() => setCreateOpen(true)}>
          <Plus className="mr-1 h-4 w-4" />
          新建一集
        </Button>
      </header>
      {episodes.isLoading && (
        <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
          <Skeleton className="h-32" />
          <Skeleton className="h-32" />
          <Skeleton className="h-32" />
        </div>
      )}
      {episodes.isError && (
        <ErrorAlert
          error={episodes.error}
          onRetry={() => episodes.refetch()}
        />
      )}
      {episodes.data?.length === 0 && (
        <EmptyState
          icon={BookOpen}
          title="还没有任何剧集"
          description="先建一集，再按分镜拆解，最后交给 AI 生成画面与视频。"
          action={
            <Button onClick={() => setCreateOpen(true)}>新建第一集</Button>
          }
        />
      )}
      {episodes.data && episodes.data.length > 0 && (
        <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
          {episodes.data.map((ep) => (
            <EpisodeCard key={ep.id} projectId={projectId} episode={ep} />
          ))}
        </div>
      )}
      <CreateEpisodeDialog
        projectId={projectId}
        open={createOpen}
        onOpenChange={setCreateOpen}
      />
    </div>
  );
}
