import { RotateCcw } from "lucide-react";
import { toast } from "sonner";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { useRestoreCheckpoint } from "@/hooks/useCheckpoint";
import type { EpisodeCheckpointListItem } from "@/lib/bindings/commands";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  checkpoint: EpisodeCheckpointListItem | null;
  episodeId: string;
  onRestored?: () => void;
}

function formatTitle(cp: EpisodeCheckpointListItem): string {
  if (cp.trigger_type === "auto") return "自动备份";
  return `版本 ${cp.version_number}${cp.label ? ` · ${cp.label}` : ""}`;
}

export function RestoreConfirmDialog({
  open,
  onOpenChange,
  checkpoint,
  episodeId,
  onRestored,
}: Props) {
  const restore = useRestoreCheckpoint(episodeId);
  const busy = restore.isPending;

  const handleConfirm = async () => {
    if (!checkpoint) return;
    try {
      await restore.mutateAsync(checkpoint.id);
      toast.success(`已恢复到${formatTitle(checkpoint)}`);
      onOpenChange(false);
      onRestored?.();
    } catch (e) {
      toast.error(`恢复失败：${String(e)}`);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[460px]">
        <DialogHeader>
          <DialogTitle>恢复版本</DialogTitle>
          <DialogDescription asChild>
            <div className="space-y-2">
              <p>
                将用「{checkpoint ? formatTitle(checkpoint) : ""}
                」覆盖当前集的所有分镜、画布与剧本。
              </p>
              <p>系统会先自动保存当前状态。</p>
              <p className="text-yellow-600 dark:text-yellow-400">
                ⚠️
                恢复后，已有的生成结果将不再关联到分镜（抽卡对比中不可见）；角色、场景、道具的分镜绑定需要重新设置。
              </p>
            </div>
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button
            variant="outline"
            type="button"
            onClick={() => onOpenChange(false)}
            disabled={busy}
          >
            取消
          </Button>
          <Button
            variant="destructive"
            type="button"
            onClick={handleConfirm}
            disabled={busy || !checkpoint}
          >
            <RotateCcw className="mr-1 h-4 w-4" />
            {busy ? "恢复中..." : "确认恢复"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
