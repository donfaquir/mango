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

export default function VideoEditorPage() {
  const { projectId, episodeId } = useParams<{
    projectId: string;
    episodeId: string;
  }>();
  const navigate = useNavigate();
  const [exportOpen, setExportOpen] = useState(false);
  const createClip = useCreateVideoClip(episodeId ?? "");

  const { data: assets, isLoading } = useAssetList(projectId, "video");

  if (!projectId || !episodeId) return <Navigate to="/" replace />;

  if (isLoading) return <Skeleton className="h-96 w-full" />;

  const videoAssets = assets ?? [];
  const firstVideo = videoAssets[0];

  if (!firstVideo) {
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
    <VideoEditorPageContent
      episodeId={episodeId}
      assetId={firstVideo.id}
      videoPath={firstVideo.file_path}
      exportOpen={exportOpen}
      setExportOpen={setExportOpen}
      onAddClip={() => {
        const { trimStart, trimEnd } = useTimelineStore.getState();
        createClip.mutate({
          project_id: projectId,
          episode_id: episodeId,
          source_asset_id: firstVideo.id,
          label: null,
          trim_start_ms: trimStart || null,
          trim_end_ms: trimEnd || null,
        });
      }}
      onBack={() => navigate(-1)}
    />
  );
}

function VideoEditorPageContent({
  episodeId,
  videoPath,
  exportOpen,
  setExportOpen,
  onAddClip,
  onBack,
}: {
  episodeId: string;
  assetId: string;
  videoPath: string;
  exportOpen: boolean;
  setExportOpen: (v: boolean) => void;
  onAddClip: () => void;
  onBack: () => void;
}) {

  return (
    <div className="flex flex-col gap-0 p-1">
      <header className="flex items-center gap-2 py-1">
        <Button variant="ghost" size="icon" onClick={onBack}>
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <span className="text-sm font-medium">视频编辑</span>
      </header>
      <VideoEditorLayout videoPath={videoPath} />
      <ClipAssembly
        episodeId={episodeId}
        onAddClip={onAddClip}
        onExport={() => setExportOpen(true)}
      />
      <ExportSettingsSheet
        open={exportOpen}
        onOpenChange={setExportOpen}
        episodeId={episodeId}
        defaultOutputPath={`exports/${episodeId}_export.mp4`}
      />
    </div>
  );
}
