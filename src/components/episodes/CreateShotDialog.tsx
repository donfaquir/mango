import { useState, type FormEvent } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { useCreateShot } from "@/hooks/useShots";

interface Props {
  episodeId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function CreateShotDialog({ episodeId, open, onOpenChange }: Props) {
  const [summary, setSummary] = useState("");
  const mutation = useCreateShot();

  const reset = () => {
    setSummary("");
    mutation.reset();
  };

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    try {
      await mutation.mutateAsync({
        episode_id: episodeId,
        summary: summary.trim() || null,
      });
      reset();
      onOpenChange(false);
    } catch (err) {
      toast.error(
        `创建失败：${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) reset();
        onOpenChange(next);
      }}
    >
      <DialogContent className="sm:max-w-[460px]">
        <DialogHeader>
          <DialogTitle>新建分镜</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="shot-summary">摘要</Label>
            <Textarea
              id="shot-summary"
              value={summary}
              onChange={(e) => setSummary(e.target.value)}
              rows={3}
              placeholder="一句话描述本镜内容（可留空）"
              maxLength={500}
            />
          </div>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              取消
            </Button>
            <Button type="submit" disabled={mutation.isPending}>
              {mutation.isPending ? "创建中..." : "创建"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
