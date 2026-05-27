import { Navigate, useParams } from "react-router-dom";
import { useProject } from "@/hooks/useProjects";
import { AssetLibraryPanel } from "@/components/assets/AssetLibraryPanel";

export default function AssetLibraryPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const { data: project } = useProject(projectId);

  if (!projectId) return <Navigate to="/" replace />;

  // Wait for project data to be available (need root_path)
  if (!project) return null;

  return <AssetLibraryPanel projectId={projectId} projectRoot={project.root_path} />;
}
