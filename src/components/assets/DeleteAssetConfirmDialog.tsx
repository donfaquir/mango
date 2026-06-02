import { Trash2 } from "lucide-react";
import type { Asset } from "@/lib/bindings/commands";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

interface Props {
  asset: Asset | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onConfirm: (asset: Asset) => void;
}

export function DeleteAssetConfirmDialog({
  asset,
  open,
  onOpenChange,
  onConfirm,
}: Props) {
  const handleConfirm = () => {
    if (!asset) return;
    onConfirm(asset);
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[420px]">
        <DialogHeader>
          <DialogTitle>删除素材</DialogTitle>
          <DialogDescription asChild>
            <div className="space-y-2">
              <p>
                确认删除
                {asset ? `「${asset.original_name}」` : "该素材"}吗？
              </p>
              <p className="text-muted-foreground">
                删除后将有 5 秒撤销时间，过期才会真正从素材库移除。
              </p>
            </div>
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
          >
            取消
          </Button>
          <Button
            type="button"
            variant="destructive"
            onClick={handleConfirm}
            disabled={!asset}
          >
            <Trash2 className="mr-1 h-4 w-4" />
            确认删除
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
