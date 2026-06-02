import { useNavigate } from "react-router-dom";
import { FolderOpen } from "lucide-react";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import type { Project } from "@/lib/bindings/commands";
import { parseDbDate } from "@/lib/datetime";

interface ProjectCardProps {
  project: Project;
}

function formatDate(dateStr: string): string {
  const date = parseDbDate(dateStr);
  return date.toLocaleDateString("zh-CN", {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

export function ProjectCard({ project }: ProjectCardProps) {
  const navigate = useNavigate();

  return (
    <Card
      className="cursor-pointer gap-3 py-5 transition-shadow hover:shadow-md"
      onClick={() => navigate(`/project/${project.id}`)}
    >
      <CardHeader className="pb-0">
        <div className="flex items-center gap-2">
          <FolderOpen size={18} className="text-primary" />
          <CardTitle className="text-base">{project.name}</CardTitle>
        </div>
        {project.description && (
          <CardDescription className="line-clamp-2">
            {project.description}
          </CardDescription>
        )}
      </CardHeader>
      <CardContent>
        <p className="text-xs text-muted-foreground">
          创建于 {formatDate(project.created_at)}
        </p>
      </CardContent>
    </Card>
  );
}
