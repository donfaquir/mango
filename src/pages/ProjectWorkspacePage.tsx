import { useParams } from "react-router-dom";

export default function ProjectWorkspacePage() {
  const { projectId } = useParams<{ projectId: string }>();
  return (
    <div className="text-muted-foreground">
      项目工作区（{projectId}）— 待实现
    </div>
  );
}
