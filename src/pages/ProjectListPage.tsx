import { useState } from "react";
import { FolderPlus, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { CreateProjectDialog } from "@/components/project/CreateProjectDialog";
import { EmptyState } from "@/components/common/EmptyState";
import { ProjectCard } from "@/components/project/ProjectCard";
import { useProjectList } from "@/hooks/useProjects";

export default function ProjectListPage() {
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const { data: projects, isLoading, error } = useProjectList();

  if (isLoading) {
    return (
      <div className="flex h-64 items-center justify-center">
        <p className="text-muted-foreground">加载中...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex h-64 items-center justify-center">
        <p className="text-destructive">
          加载失败：{error instanceof Error ? error.message : String(error)}
        </p>
      </div>
    );
  }

  const count = projects?.length ?? 0;
  const hasProjects = !!projects && count > 0;

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold">我的项目</h2>
          <p className="text-muted-foreground">{count} 个项目</p>
        </div>
        <Button onClick={() => setCreateDialogOpen(true)}>
          <Plus size={16} className="mr-2" />
          新建项目
        </Button>
      </div>

      {hasProjects ? (
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {projects.map((project) => (
            <ProjectCard key={project.id} project={project} />
          ))}
        </div>
      ) : (
        <EmptyState
          icon={FolderPlus}
          title="还没有项目"
          description="创建你的第一个漫剧项目，开始创作之旅"
          action={
            <Button onClick={() => setCreateDialogOpen(true)}>新建项目</Button>
          }
        />
      )}

      <CreateProjectDialog
        open={createDialogOpen}
        onOpenChange={setCreateDialogOpen}
      />
    </div>
  );
}
