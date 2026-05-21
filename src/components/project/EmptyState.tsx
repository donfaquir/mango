import { FolderPlus } from "lucide-react";
import { Button } from "@/components/ui/button";

interface EmptyStateProps {
  onCreateClick: () => void;
}

export function EmptyState({ onCreateClick }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center rounded-lg border-2 border-dashed p-12">
      <FolderPlus size={48} className="text-muted-foreground/50" />
      <h3 className="mt-4 text-lg font-medium">还没有项目</h3>
      <p className="mt-2 text-sm text-muted-foreground">
        创建你的第一个漫剧项目，开始创作之旅
      </p>
      <Button className="mt-6" onClick={onCreateClick}>
        新建项目
      </Button>
    </div>
  );
}
