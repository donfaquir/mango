import { useState } from "react";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { useAsset } from "@/hooks/useAssets";
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

  // Fetch the actual asset to get its real file_path (avoids hardcoded extensions)
  const { data: asset } = useAsset(task.result_asset_id);
  const url = useResolvedAssetUrl(project.data?.root_path, asset?.file_path ?? null);

  const isVideo = asset?.asset_type === "video";
  const isAudio = asset?.asset_type === "audio";

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
        ) : isAudio ? (
          <div className="flex flex-col items-center gap-4 p-6">
            <p className="text-sm text-muted-foreground">音频预览</p>
            <audio
              src={url}
              controls
              autoPlay
              className="w-full"
              onError={() => setMediaError(true)}
            />
          </div>
        ) : isVideo ? (
          <video
            src={url}
            controls
            autoPlay
            className="w-full max-h-[80vh]"
            onError={() => setMediaError(true)}
          />
        ) : (
          <img
            src={url}
            alt=""
            className="w-full max-h-[80vh] object-contain"
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
