import { useState } from "react";
import { Navigate, useNavigate, useParams } from "react-router-dom";
import { ArrowLeft } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { VideoEditorLayout } from "@/components/editor/VideoEditorLayout";
import { ClipAssembly } from "@/components/editor/ClipAssembly";
import { ExportSettingsSheet } from "@/components/editor/ExportSettingsSheet";
import { useCreateVideoClip } from "@/hooks/useVideoClips";
import { useTimelineStore } from "@/stores/timelineStore";
import { useAssetList } from "@/hooks/useAssets";
import { useProject } from "@/hooks/useProjects";
import { useResolvedAssetUrl } from "@/hooks/useResolvedAssetUrl";
import { useResolvedPath } from "@/hooks/useResolvedPath";

export default function VideoEditorPage() {
  const { projectId, episodeId } = useParams<{
    projectId: string;
    episodeId: string;
  }>();
  const navigate = useNavigate();
  const [exportOpen, setExportOpen] = useState(false);
  const createClip = useCreateVideoClip(episodeId ?? "");

  const { data: project } = useProject(projectId);
  const { data: assets, isLoading } = useAssetList(projectId, "video");

  const projectRoot = project?.root_path;
  const videoAssets = assets ?? [];
  const firstVideo = videoAssets[0];

  const videoSrcUrl = useResolvedAssetUrl(projectRoot, firstVideo?.file_path ?? null);
  const absoluteVideoPath = useResolvedPath(projectRoot, firstVideo?.file_path ?? null);
  const defaultExportPath = useResolvedPath(projectRoot, `exports/${episodeId}_export.mp4`);

  if (!projectId || !episodeId) return <Navigate to="/" replace />;
  if (isLoading) return <Skeleton className="h-96 w-full" />;

  if (!firstVideo || !projectRoot || !absoluteVideoPath) {
    return (
      <div className="flex flex-col items-center gap-4 py-16">
        <p className="text-muted-foreground">该项目暂无视频素材</p>
        <Button variant="outline" onClick={() => navigate(-1)}>
          返回
        </Button>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-0 p-1">
      <header className="flex items-center gap-2 py-1">
        <Button variant="ghost" size="icon" onClick={() => navigate(-1)}>
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <span className="text-sm font-medium">视频编辑</span>
      </header>
      <VideoEditorLayout
        videoPath={absoluteVideoPath}
        videoSrcUrl={videoSrcUrl}
      />
      <ClipAssembly
        episodeId={episodeId}
        onAddClip={() => {
          const { trimStart, trimEnd, duration } = useTimelineStore.getState();
          const hasCustomTrim = trimStart > 0 || trimEnd < duration;
          createClip.mutate({
            project_id: projectId,
            episode_id: episodeId,
            source_asset_id: firstVideo.id,
            label: null,
            trim_start_ms: hasCustomTrim ? trimStart : null,
            trim_end_ms: hasCustomTrim ? trimEnd : null,
          });
        }}
        onExport={() => setExportOpen(true)}
      />
      <ExportSettingsSheet
        open={exportOpen}
        onOpenChange={setExportOpen}
        episodeId={episodeId}
        defaultOutputPath={defaultExportPath ?? ""}
      />
    </div>
  );
}
