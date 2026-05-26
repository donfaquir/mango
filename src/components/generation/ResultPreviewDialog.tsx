import { useState } from "react";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { useResolvedAssetUrl } from "@/hooks/useResolvedAssetUrl";
import { useProject } from "@/hooks/useProjects";
import type { GenerationTask } from "@/lib/bindings/commands";

interface ResultPreviewDialogProps {
  task: GenerationTask;
  projectId: string;
  onClose: () => void;
}

export function ResultPreviewDialog({
  task,
  projectId,
  onClose,
}: ResultPreviewDialogProps) {
  const project = useProject(projectId);
  const [mediaError, setMediaError] = useState(false);

  // Parse params_json to get prompt for display
  let prompt = "";
  try {
    const params = JSON.parse(task.params_json);
    prompt = params.prompt ?? "";
  } catch {
    // ignore
  }

  // result_asset_id links to an asset; for preview we use the asset path
  // Since GenerationTask doesn't carry result_asset_path directly, we use
  // a convention: output is stored at assets/<result_asset_id>.<ext>
  // For now, we'll resolve via the task's result_asset_id field
  const resultPath = task.result_asset_id
    ? `assets/${task.result_asset_id}.${task.task_type === "video" ? "mp4" : "png"}`
    : null;

  const url = useResolvedAssetUrl(project.data?.root_path, resultPath);

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-4xl">
        {mediaError ? (
          <p className="p-6 text-center text-sm text-muted-foreground">
            结果文件不在了，可能已被手动删除
          </p>
        ) : !url ? (
          <p className="p-6 text-center text-sm text-muted-foreground">
            加载中…
          </p>
        ) : task.task_type === "image" ? (
          <img
            src={url}
            alt=""
            className="w-full max-h-[80vh] object-contain"
            onError={() => setMediaError(true)}
          />
        ) : (
          <video
            src={url}
            controls
            autoPlay
            className="w-full max-h-[80vh]"
            onError={() => setMediaError(true)}
          />
        )}
        {prompt && (
          <p className="px-4 pb-3 text-sm text-muted-foreground line-clamp-3">
            {prompt}
          </p>
        )}
      </DialogContent>
    </Dialog>
  );
}
