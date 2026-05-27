import { Navigate, useParams } from "react-router-dom";
import { SubmitTaskPanel } from "@/components/generation/SubmitTaskPanel";
import { TaskListPanel } from "@/components/generation/TaskListPanel";

export default function GenerationWorkspacePage() {
  const { projectId } = useParams<{ projectId: string }>();

  if (!projectId) return <Navigate to="/" replace />;

  return (
    <div className="grid h-full grid-cols-[minmax(360px,1fr)_2fr] gap-6">
      <SubmitTaskPanel projectId={projectId} />
      <TaskListPanel projectId={projectId} />
    </div>
  );
}
