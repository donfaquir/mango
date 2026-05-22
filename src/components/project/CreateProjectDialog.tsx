import { useEffect, useState, type FormEvent } from "react";
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
import { Textarea } from "@/components/ui/textarea";
import { useCreateProject } from "@/hooks/useProjects";
import { commands } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

interface CreateProjectDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function CreateProjectDialog({
  open,
  onOpenChange,
}: CreateProjectDialogProps) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [stylePrompt, setStylePrompt] = useState("");
  const [rootPath, setRootPath] = useState("");
  const [rootPathEdited, setRootPathEdited] = useState(false);

  const createProject = useCreateProject();

  const reset = () => {
    setName("");
    setDescription("");
    setStylePrompt("");
    setRootPath("");
    setRootPathEdited(false);
    createProject.reset();
  };

  const handleClose = (next: boolean) => {
    if (!next) reset();
    onOpenChange(next);
  };

  // Re-suggest a root path whenever the name changes, unless the user has
  // edited the path field themselves (we don't want to clobber their pick).
  useEffect(() => {
    if (rootPathEdited) return;
    const trimmed = name.trim();
    if (!trimmed) {
      setRootPath("");
      return;
    }
    let cancelled = false;
    unwrap(commands.suggestProjectRoot(trimmed))
      .then((suggested) => {
        if (!cancelled) setRootPath(suggested);
      })
      .catch(() => {
        // Suggestion is best-effort; leave the field as-is on failure.
      });
    return () => {
      cancelled = true;
    };
  }, [name, rootPathEdited]);

  const handlePick = async () => {
    try {
      const picked = await unwrap(commands.pickProjectDirectory());
      if (picked) {
        setRootPath(picked);
        setRootPathEdited(true);
      }
    } catch (err) {
      console.error("pick directory failed", err);
    }
  };

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    const trimmedName = name.trim();
    const trimmedRoot = rootPath.trim();
    if (!trimmedName || !trimmedRoot) return;

    await createProject.mutateAsync({
      name: trimmedName,
      root_path: trimmedRoot,
      description: description.trim() || null,
      style_prompt: stylePrompt.trim() || null,
      global_seed: null,
    });

    reset();
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={handleClose}>
      <DialogContent className="sm:max-w-[480px]">
        <DialogHeader>
          <DialogTitle>新建项目</DialogTitle>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="project-name">项目名称 *</Label>
            <Input
              id="project-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="如：我的第一部漫剧"
              autoFocus
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="project-root">项目位置 *</Label>
            <div className="flex gap-2">
              <Input
                id="project-root"
                value={rootPath}
                onChange={(e) => {
                  setRootPath(e.target.value);
                  setRootPathEdited(true);
                }}
                placeholder="项目目录路径"
              />
              <Button type="button" variant="outline" onClick={handlePick}>
                选择
              </Button>
            </div>
            <p className="text-xs text-muted-foreground">
              将自动创建 assets/ 和 thumbnails/ 子目录。
            </p>
          </div>

          <div className="space-y-2">
            <Label htmlFor="project-description">项目描述</Label>
            <Textarea
              id="project-description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="简要描述项目内容..."
              rows={3}
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="project-style">风格提示词</Label>
            <Textarea
              id="project-style"
              value={stylePrompt}
              onChange={(e) => setStylePrompt(e.target.value)}
              placeholder="如：赛博朋克风格，霓虹灯光，暗色调..."
              rows={2}
            />
          </div>

          {createProject.error && (
            <p className="text-sm text-destructive">
              创建失败：
              {createProject.error instanceof Error
                ? createProject.error.message
                : String(createProject.error)}
            </p>
          )}

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => handleClose(false)}
            >
              取消
            </Button>
            <Button
              type="submit"
              disabled={
                !name.trim() || !rootPath.trim() || createProject.isPending
              }
            >
              {createProject.isPending ? "创建中..." : "创建"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
