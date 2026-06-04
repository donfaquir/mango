import { useState } from "react";
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
import { Skeleton } from "@/components/ui/skeleton";
import { useCheckpointList, useDeleteCheckpoint } from "@/hooks/useCheckpoint";
import type { EpisodeCheckpointListItem } from "@/lib/bindings/commands";
import { CheckpointListItem } from "./CheckpointListItem";
import { RestoreConfirmDialog } from "./RestoreConfirmDialog";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  episodeId: string;
}

export function VersionHistoryDialog({
  open,
  onOpenChange,
  episodeId,
}: Props) {
  const { data: items, isLoading } = useCheckpointList(
    open ? episodeId : undefined,
  );
  const deleteCheckpoint = useDeleteCheckpoint(episodeId);

  const [restoreTarget, setRestoreTarget] =
    useState<EpisodeCheckpointListItem | null>(null);
  const [deleteTarget, setDeleteTarget] =
    useState<EpisodeCheckpointListItem | null>(null);

  const handleDelete = async () => {
    if (!deleteTarget) return;
    try {
      await deleteCheckpoint.mutateAsync(deleteTarget.id);
      toast.success("已删除");
      setDeleteTarget(null);
    } catch (e) {
      toast.error(`删除失败：${String(e)}`);
    }
  };

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-[520px]">
          <DialogHeader>
            <DialogTitle>版本历史</DialogTitle>
          </DialogHeader>
          {isLoading && (
            <div className="space-y-2">
              <Skeleton className="h-16 w-full" />
              <Skeleton className="h-16 w-full" />
            </div>
          )}
          {!isLoading && items?.length === 0 && (
            <p className="py-8 text-center text-sm text-muted-foreground">
              暂无版本记录
            </p>
          )}
          {!isLoading && items && items.length > 0 && (
            <div className="space-y-2">
              {items.map((item) => (
                <CheckpointListItem
                  key={item.id}
                  item={item}
                  onRestore={setRestoreTarget}
                  onDelete={setDeleteTarget}
                />
              ))}
            </div>
          )}
        </DialogContent>
      </Dialog>

      <RestoreConfirmDialog
        open={restoreTarget !== null}
        onOpenChange={(v) => { if (!v) setRestoreTarget(null); }}
        checkpoint={restoreTarget}
        episodeId={episodeId}
        onRestored={() => onOpenChange(false)}
      />

      <Dialog
        open={deleteTarget !== null}
        onOpenChange={(v) => { if (!v) setDeleteTarget(null); }}
      >
        <DialogContent className="sm:max-w-[420px]">
          <DialogHeader>
            <DialogTitle>
              {deleteTarget?.trigger_type === "manual"
                ? "删除版本"
                : "删除自动备份"}
            </DialogTitle>
            <DialogDescription>
              {deleteTarget?.trigger_type === "manual"
                ? `删除「版本 ${deleteTarget.version_number}${deleteTarget.label ? ` · ${deleteTarget.label}` : ""}」后不可恢复，确认删除？`
                : "删除此自动备份记录？"}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="outline"
              type="button"
              onClick={() => setDeleteTarget(null)}
              disabled={deleteCheckpoint.isPending}
            >
              取消
            </Button>
            <Button
              variant="destructive"
              type="button"
              onClick={handleDelete}
              disabled={deleteCheckpoint.isPending}
            >
              {deleteCheckpoint.isPending ? "删除中..." : "确认删除"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
