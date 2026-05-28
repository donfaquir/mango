import { useState, type FormEvent } from "react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useCreateEpisode } from "@/hooks/useEpisodes";

interface Props {
  projectId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function CreateEpisodeDialog({ projectId, open, onOpenChange }: Props) {
  const [title, setTitle] = useState("");
  const navigate = useNavigate();
  const mutation = useCreateEpisode();

  const reset = () => {
    setTitle("");
    mutation.reset();
  };

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    const trimmed = title.trim();
    if (!trimmed) return;
    try {
      const created = await mutation.mutateAsync({
        project_id: projectId,
        title: trimmed,
        script_text: null,
      });
      reset();
      onOpenChange(false);
      navigate(`/project/${projectId}/episodes/${created.id}`);
    } catch (err) {
      toast.error(
        `创建失败：${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  const disabled = mutation.isPending || !title.trim();

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) reset();
        onOpenChange(next);
      }}
    >
      <DialogContent className="sm:max-w-[420px]">
        <DialogHeader>
          <DialogTitle>新建一集</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="episode-title">标题 *</Label>
            <Input
              id="episode-title"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="例如：第一幕：开学"
              autoFocus
              maxLength={200}
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
            <Button type="submit" disabled={disabled}>
              {mutation.isPending ? "创建中..." : "创建"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
