import { useCallback } from "react";
import { toast } from "sonner";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import type { Shot } from "@/lib/bindings/commands";
import { useAsset, useAssignAssetToShot } from "@/hooks/useAssets";
import { useShot } from "@/hooks/useShots";

export interface BindAssetToShotRequest {
  assetId: string;
  shotId: string;
}

interface Props {
  request: BindAssetToShotRequest | null;
  onClose: () => void;
}

function shotLabel(shot: Shot | undefined): string {
  if (!shot) return "";
  const summary = shot.summary.trim();
  if (summary.length > 0) return summary;
  return `Shot ${shot.order_index + 1}`;
}

export function BindAssetToShotDialog({ request, onClose }: Props) {
  const { data: asset } = useAsset(request?.assetId);
  const { data: targetShot } = useShot(request?.shotId);
  const { data: currentShot } = useShot(asset?.shot_id ?? undefined);
  const assign = useAssignAssetToShot();

  const handleConfirm = useCallback(async () => {
    if (!request || !asset) return;
    try {
      await assign.mutateAsync({
        id: request.assetId,
        shotId: request.shotId,
      });
      toast.success(
        asset.asset_type === "video"
          ? "已采用为分镜的视频结果"
          : "已设为分镜的参考素材",
      );
      onClose();
    } catch (err) {
      toast.error(
        `绑定失败：${err instanceof Error ? err.message : String(err)}`,
      );
    }
  }, [assign, asset, request, onClose]);

  if (!request) return null;

  const ready = asset != null && targetShot != null;
  const alreadyBoundElsewhere =
    asset != null &&
    asset.shot_id != null &&
    asset.shot_id !== request.shotId;

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>绑定素材到分镜</DialogTitle>
        </DialogHeader>
        {ready ? (
          <div className="space-y-3 text-sm">
            <p className="text-muted-foreground">
              将素材 <b className="text-foreground">{asset.original_name}</b>{" "}
              绑定到分镜{" "}
              <b className="text-foreground">{shotLabel(targetShot)}</b>。
            </p>
            {alreadyBoundElsewhere && (
              <div className="rounded-md border border-amber-300 bg-amber-50 p-3 text-amber-900">
                该素材当前已绑定到{" "}
                <b>{shotLabel(currentShot) || asset.shot_id}</b>
                ，继续操作会覆盖原绑定。
              </div>
            )}
          </div>
        ) : (
          <p className="text-sm text-muted-foreground">加载中…</p>
        )}
        <DialogFooter>
          <Button variant="outline" onClick={onClose} disabled={assign.isPending}>
            取消
          </Button>
          <Button
            onClick={handleConfirm}
            disabled={!ready || assign.isPending}
          >
            {alreadyBoundElsewhere ? "覆盖并绑定" : "确认绑定"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
