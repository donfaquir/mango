import { useState } from "react";
import { Navigate, useNavigate, useParams } from "react-router-dom";
import { ArrowLeft } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { MultiTrackEditor } from "@/components/editor/MultiTrackEditor";
import { ExportSettingsSheet } from "@/components/editor/ExportSettingsSheet";
import { useProject } from "@/hooks/useProjects";
import { useResolvedPath } from "@/hooks/useResolvedPath";

export default function VideoEditorPage() {
  const { projectId, episodeId } = useParams<{
    projectId: string;
    episodeId: string;
  }>();
  const navigate = useNavigate();
  const [exportOpen, setExportOpen] = useState(false);

  const { data: project, isLoading } = useProject(projectId);
  const projectRoot = project?.root_path;
  const defaultExportPath = useResolvedPath(projectRoot, `exports/${episodeId}_export.mp4`);

  if (!projectId || !episodeId) return <Navigate to="/" replace />;
  if (isLoading) return <Skeleton className="h-96 w-full" />;

  return (
    <div className="flex flex-col gap-0 p-1">
      <header className="flex items-center gap-2 py-1">
        <Button variant="ghost" size="icon" onClick={() => navigate(-1)}>
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <span className="text-sm font-medium">Timeline Editor</span>
        <Button
          variant="outline"
          size="sm"
          className="ml-auto"
          onClick={() => setExportOpen(true)}
        >
          Export
        </Button>
      </header>
      <MultiTrackEditor episodeId={episodeId} />
      <ExportSettingsSheet
        open={exportOpen}
        onOpenChange={setExportOpen}
        episodeId={episodeId}
        defaultOutputPath={defaultExportPath ?? ""}
      />
    </div>
  );
}
