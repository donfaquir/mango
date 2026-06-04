import { useState } from "react";
import { Check, Eye, X } from "lucide-react";
import { toast } from "sonner";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useTaskList } from "@/hooks/useTasks";
import { useAdoptTaskResult, useUnadoptShot } from "@/hooks/useShots";
import { useAsset } from "@/hooks/useAssets";
import { useResolvedAssetUrl } from "@/hooks/useResolvedAssetUrl";
import { useProject } from "@/hooks/useProjects";
import { ResultPreviewDialog } from "./ResultPreviewDialog";
import type { GenerationTask } from "@/lib/bindings/commands";
import { formatRelative } from "@/lib/datetime";

interface Props {
  shotId: string;
  episodeId: string;
  projectId: string;
  adoptedAssetId: string | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function GachaCompareDialog({
  shotId,
  episodeId,
  projectId,
  adoptedAssetId,
  open,
  onOpenChange,
}: Props) {
  const tasks = useTaskList(projectId, "success", undefined, shotId);
  const adopt = useAdoptTaskResult(episodeId);
  const unadopt = useUnadoptShot(episodeId);
  const [previewTask, setPreviewTask] = useState<GenerationTask | null>(null);

  const handleAdopt = async (task: GenerationTask) => {
    try {
      await adopt.mutateAsync({ shotId, taskId: task.id });
      toast.success("已采用为分镜结果");
    } catch (err) {
      toast.error(`采用失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const handleUnadopt = async () => {
    try {
      await unadopt.mutateAsync(shotId);
      toast.success("已取消采用");
    } catch (err) {
      toast.error(`取消失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="sm:max-w-3xl max-h-[85vh] overflow-y-auto">
          <DialogHeader>
            <DialogTitle>对比生成结果</DialogTitle>
          </DialogHeader>
          {tasks.data?.length === 0 && (
            <p className="py-8 text-center text-sm text-muted-foreground">
              暂无成功的生成结果
            </p>
          )}
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3">
            {tasks.data?.map((task) => (
              <CompareCard
                key={task.id}
                task={task}
                projectId={projectId}
                isAdopted={task.result_asset_id === adoptedAssetId && adoptedAssetId !== null}
                onPreview={() => setPreviewTask(task)}
                onAdopt={() => handleAdopt(task)}
                onUnadopt={handleUnadopt}
                adopting={adopt.isPending}
              />
            ))}
          </div>
        </DialogContent>
      </Dialog>
      {previewTask && (
        <ResultPreviewDialog
          task={previewTask}
          projectId={projectId}
          onClose={() => setPreviewTask(null)}
        />
      )}
    </>
  );
}

interface CompareCardProps {
  task: GenerationTask;
  projectId: string;
  isAdopted: boolean;
  onPreview: () => void;
  onAdopt: () => void;
  onUnadopt: () => void;
  adopting: boolean;
}

function CompareCard({
  task,
  projectId,
  isAdopted,
  onPreview,
  onAdopt,
  onUnadopt,
  adopting,
}: CompareCardProps) {
  const { data: asset } = useAsset(task.result_asset_id);
  const { data: project } = useProject(projectId);
  const thumbRelPath = asset?.thumbnail_path ?? asset?.file_path ?? null;
  const thumbUrl = useResolvedAssetUrl(project?.root_path, thumbRelPath);

  return (
    <div
      className={cn(
        "flex flex-col rounded-lg border bg-card overflow-hidden",
        isAdopted && "ring-2 ring-primary",
      )}
    >
      <button
        type="button"
        className="relative aspect-square bg-muted group"
        onClick={onPreview}
      >
        {thumbUrl ? (
          <>
            <img src={thumbUrl} alt="" className="h-full w-full object-cover" />
            <div className="absolute inset-0 flex items-center justify-center bg-black/0 group-hover:bg-black/30 transition-colors">
              <Eye className="size-5 text-white opacity-0 group-hover:opacity-100 transition-opacity" />
            </div>
          </>
        ) : (
          <div className="flex h-full w-full items-center justify-center text-xs text-muted-foreground">
            加载中…
          </div>
        )}
        {isAdopted && (
          <div className="absolute top-1.5 right-1.5 rounded-full bg-primary p-1">
            <Check className="size-3 text-primary-foreground" />
          </div>
        )}
      </button>
      <div className="flex flex-col gap-1.5 p-2">
        <div className="flex items-center justify-between">
          <span className="text-xs font-medium truncate">{task.model_id}</span>
          <span className="text-xs text-muted-foreground shrink-0">
            {formatRelative(task.created_at)}
          </span>
        </div>
        {isAdopted ? (
          <Button
            size="sm"
            variant="outline"
            className="w-full"
            onClick={onUnadopt}
            disabled={adopting}
          >
            <X className="mr-1 h-3 w-3" />
            取消采用
          </Button>
        ) : (
          <Button
            size="sm"
            className="w-full"
            onClick={onAdopt}
            disabled={adopting}
          >
            <Check className="mr-1 h-3 w-3" />
            采用
          </Button>
        )}
      </div>
    </div>
  );
}
