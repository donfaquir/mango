import { NavLink, useParams } from "react-router-dom";
import { BookOpen, FolderOpen, Users, Wand2 } from "lucide-react";
import { cn } from "@/lib/utils";

const navItems = [
  {
    path: "subjects/character",
    label: "主体库",
    icon: Users,
  },
  {
    path: "episodes",
    label: "剧本",
    icon: BookOpen,
  },
  {
    path: "assets",
    label: "素材库",
    icon: FolderOpen,
  },
  {
    path: "generation",
    label: "AI 生成",
    icon: Wand2,
  },
] as const;

export function WorkspaceNav() {
  const { projectId } = useParams<{ projectId: string }>();
  if (!projectId) return null;

  return (
    <nav className="flex gap-1 border-b px-4 pb-2">
      {navItems.map((item) => (
        <NavLink
          key={item.path}
          to={`/project/${projectId}/${item.path}`}
          className={({ isActive }) =>
            cn(
              "inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm font-medium transition-colors",
              isActive
                ? "bg-muted text-foreground"
                : "text-muted-foreground hover:text-foreground",
            )
          }
        >
          <item.icon className="h-4 w-4" />
          {item.label}
        </NavLink>
      ))}
    </nav>
  );
}
