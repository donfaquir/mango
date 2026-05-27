import { useState, useEffect } from "react";
import { Check } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { useCharacterList } from "@/hooks/useCharacters";
import { SubjectThumbnail } from "@/components/subjects/SubjectThumbnail";
import { cn } from "@/lib/utils";

interface SubjectPickerDialogProps {
  projectId: string;
  selected: string[];
  onChange: (ids: string[]) => void;
}

export function SubjectPickerDialog({
  projectId,
  selected,
  onChange,
}: SubjectPickerDialogProps) {
  const [open, setOpen] = useState(false);
  const characters = useCharacterList(projectId);
  const [draft, setDraft] = useState<string[]>(selected);

  useEffect(() => {
    if (open) setDraft(selected);
  }, [open, selected]);

  const toggle = (id: string) =>
    setDraft((prev) =>
      prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id],
    );

  // Only show characters that have a reference image
  const availableCharacters = (characters.data ?? []).filter(
    (c) => c.reference_image_path,
  );

  return (
    <>
      <Button type="button" variant="outline" onClick={() => setOpen(true)}>
        {selected.length === 0 ? "选择主体" : `已选 ${selected.length} 个`}
      </Button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="sm:max-w-3xl">
          <DialogTitle>选择主体</DialogTitle>
          {availableCharacters.length === 0 ? (
            <p className="py-8 text-center text-sm text-muted-foreground">
              当前项目没有带参考图的角色。请先在主体库中添加角色并上传参考图。
            </p>
          ) : (
            <div className="grid grid-cols-3 gap-3 max-h-[60vh] overflow-auto">
              {availableCharacters.map((c) => (
                <button
                  key={c.id}
                  type="button"
                  onClick={() => toggle(c.id)}
                  className={cn(
                    "relative flex flex-col gap-2 rounded-lg border p-3 text-left transition-colors hover:bg-accent",
                    draft.includes(c.id) && "border-primary ring-2 ring-primary/20",
                  )}
                >
                  {draft.includes(c.id) && (
                    <div className="absolute top-2 right-2 rounded-full bg-primary p-0.5">
                      <Check className="h-3 w-3 text-primary-foreground" />
                    </div>
                  )}
                  <SubjectThumbnail
                    projectId={projectId}
                    relativePath={c.reference_image_path}
                  />
                  <span className="truncate text-sm font-medium">{c.name}</span>
                </button>
              ))}
            </div>
          )}
          <DialogFooter>
            <Button variant="outline" onClick={() => setOpen(false)}>
              取消
            </Button>
            <Button
              onClick={() => {
                onChange(draft);
                setOpen(false);
              }}
            >
              确定（{draft.length}）
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
