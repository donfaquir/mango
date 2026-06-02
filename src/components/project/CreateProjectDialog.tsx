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

interface CreateProjectDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

// Frontend-side slug for the default subdir field. Preserves unicode
// (CJK names like "我的第一部漫剧" become "我的第一部漫剧") because all
// three target filesystems (macOS APFS, Linux ext/btrfs, Windows NTFS)
// store UTF-8 directory names natively. Only the characters that are
// genuinely unsafe for a directory name are stripped — the backend
// (`validate_subdir_segment` in crates/core/src/paths.rs) re-checks the
// same restrictions on submit.
function slugify(s: string): string {
  return s
    .trim()
    .replace(/\s+/g, "-")
    // Path separators + Windows-reserved chars + NUL.
    .replace(/[/\\:*?"<>|\0]/g, "")
    // Unicode control characters (the regex `\p{C}` covers them all).
    .replace(/\p{C}/gu, "")
    // No leading/trailing `.` (POSIX hidden) or `-` (looks like a flag).
    .replace(/^[.-]+|[.-]+$/g, "");
}

export function CreateProjectDialog({
  open,
  onOpenChange,
}: CreateProjectDialogProps) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [stylePrompt, setStylePrompt] = useState("");
  const [subdir, setSubdir] = useState("");
  const [subdirEdited, setSubdirEdited] = useState(false);

  const createProject = useCreateProject();

  const reset = () => {
    setName("");
    setDescription("");
    setStylePrompt("");
    setSubdir("");
    setSubdirEdited(false);
    createProject.reset();
  };

  const handleClose = (next: boolean) => {
    if (!next) reset();
    onOpenChange(next);
  };

  // Pre-fill subdir from name unless the user has already edited it. The
  // backend appends a short uuid suffix to the actual on-disk slot so even
  // an empty slug here resolves to a unique directory.
  useEffect(() => {
    if (subdirEdited) return;
    setSubdir(slugify(name));
  }, [name, subdirEdited]);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    const trimmedName = name.trim();
    if (!trimmedName) return;

    const trimmedSubdir = subdir.trim();
    await createProject.mutateAsync({
      name: trimmedName,
      subdir: trimmedSubdir.length > 0 ? trimmedSubdir : null,
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
            <Label htmlFor="project-subdir">项目子目录名</Label>
            <Input
              id="project-subdir"
              value={subdir}
              onChange={(e) => {
                setSubdir(e.target.value);
                setSubdirEdited(true);
              }}
              placeholder="自动从名称生成"
            />
            <p className="text-xs text-muted-foreground">
              将创建为 <code>&lt;工作区&gt;/projects/{subdir || "{slug}"}/</code>，
              内含 assets/ 和 thumbnails/ 子目录。
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
              disabled={!name.trim() || createProject.isPending}
            >
              {createProject.isPending ? "创建中..." : "创建"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
