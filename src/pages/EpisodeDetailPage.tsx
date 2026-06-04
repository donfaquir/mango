import { useState } from "react";
import { Link, Navigate, useNavigate, useParams } from "react-router-dom";
import { toast } from "sonner";
import { ArrowLeft, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { ErrorAlert } from "@/components/common/ErrorAlert";
import {
  useDeleteEpisode,
  useEpisode,
  useUpdateEpisode,
} from "@/hooks/useEpisodes";
import { ScriptEditor } from "@/components/episodes/ScriptEditor";
import { ShotListPanel } from "@/components/episodes/ShotListPanel";

export default function EpisodeDetailPage() {
  const { projectId, episodeId } = useParams<{
    projectId: string;
    episodeId: string;
  }>();
  const navigate = useNavigate();
  const episode = useEpisode(episodeId);
  const updateEpisode = useUpdateEpisode(projectId ?? "");
  const deleteEpisode = useDeleteEpisode(projectId ?? "");
  const [titleDraft, setTitleDraft] = useState<string | null>(null);

  if (!projectId || !episodeId) return <Navigate to="/" replace />;

  if (episode.isLoading) {
    return <Skeleton className="h-64 w-full" />;
  }
  if (episode.isError || !episode.data) {
    return (
      <ErrorAlert
        error={episode.error ?? new Error("剧集不存在")}
        onRetry={() => episode.refetch()}
      />
    );
  }

  const ep = episode.data;
  const currentTitle = titleDraft ?? ep.title;

  const handleTitleBlur = async () => {
    const trimmed = currentTitle.trim();
    if (!trimmed || trimmed === ep.title) {
      setTitleDraft(null);
      return;
    }
    try {
      await updateEpisode.mutateAsync({
        id: ep.id,
        input: { title: trimmed },
      });
    } catch (err) {
      toast.error(
        `保存失败：${err instanceof Error ? err.message : String(err)}`,
      );
    } finally {
      setTitleDraft(null);
    }
  };

  const handleDelete = async () => {
    if (!window.confirm(`确认删除「${ep.title}」？所有分镜将被一并删除。`))
      return;
    try {
      await deleteEpisode.mutateAsync(ep.id);
      toast.success("剧集已删除");
      navigate(`/project/${projectId}/episodes`);
    } catch (err) {
      toast.error(
        `删除失败：${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  return (
    <div className="flex flex-col gap-6 p-1">
      <header className="flex items-center gap-3">
        <Button
          type="button"
          variant="ghost"
          size="icon"
          onClick={() => navigate(`/project/${projectId}/episodes`)}
          aria-label="返回剧本列表"
        >
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <Input
          value={currentTitle}
          onChange={(e) => setTitleDraft(e.target.value)}
          onBlur={handleTitleBlur}
          className="max-w-md text-lg font-semibold"
          maxLength={200}
          aria-label="剧集标题"
        />
        <div className="ml-auto flex items-center gap-2">
          <Button asChild type="button" variant="outline">
            <Link to={`/project/${projectId}/episodes/${ep.id}/canvas`}>
              打开画布
            </Link>
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            onClick={handleDelete}
            disabled={deleteEpisode.isPending}
            aria-label="删除剧集"
          >
            <Trash2 className="h-4 w-4" />
          </Button>
        </div>
      </header>
      <ScriptEditor
        projectId={projectId}
        episodeId={ep.id}
        initialText={ep.script_text}
      />
      <ShotListPanel episodeId={ep.id} projectId={projectId} />
    </div>
  );
}
