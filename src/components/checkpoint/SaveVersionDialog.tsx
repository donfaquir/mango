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
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useCreateCheckpoint } from "@/hooks/useCheckpoint";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  episodeId: string;
  onBeforeSave?: () => Promise<void> | void;
}

export function SaveVersionDialog({
  open,
  onOpenChange,
  episodeId,
  onBeforeSave,
}: Props) {
  const [label, setLabel] = useState("");
  const createCheckpoint = useCreateCheckpoint();
  const busy = createCheckpoint.isPending;

  const handleSave = async () => {
    try {
      await onBeforeSave?.();
      await createCheckpoint.mutateAsync({
        episode_id: episodeId,
        label: label.trim() || null,
      });
      toast.success("已保存版本");
      setLabel("");
      onOpenChange(false);
    } catch (e) {
      toast.error(`保存版本失败：${String(e)}`);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(v) => {
        if (!v) setLabel("");
        onOpenChange(v);
      }}
    >
      <DialogContent className="sm:max-w-[420px]">
        <DialogHeader>
          <DialogTitle>保存版本</DialogTitle>
          <DialogDescription>
            为当前状态创建一个版本快照，方便后续恢复。
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-2">
          <Label htmlFor="checkpoint-label">版本标签</Label>
          <Input
            id="checkpoint-label"
            placeholder="如：分镜定稿"
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            disabled={busy}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                handleSave();
              }
            }}
          />
        </div>
        <DialogFooter>
          <Button
            variant="outline"
            type="button"
            onClick={() => onOpenChange(false)}
            disabled={busy}
          >
            取消
          </Button>
          <Button type="button" onClick={handleSave} disabled={busy}>
            {busy ? "保存中..." : "保存版本"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
