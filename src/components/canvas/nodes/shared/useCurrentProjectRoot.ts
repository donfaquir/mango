import { useParams } from "react-router-dom";
import { useProject } from "@/hooks/useProjects";

export function useCurrentProjectRoot(): string | undefined {
  const { projectId } = useParams<{ projectId: string }>();
  const { data: project } = useProject(projectId);
  return project?.root_path;
}
