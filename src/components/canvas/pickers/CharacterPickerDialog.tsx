import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Loader2, Search, User } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useCharacterList } from "@/hooks/useCharacters";
import { cn } from "@/lib/utils";

interface Props {
  open: boolean;
  projectId: string | undefined;
  onOpenChange: (open: boolean) => void;
  onSelect: (characterId: string) => void;
}

export function CharacterPickerDialog({
  open,
  projectId,
  onOpenChange,
  onSelect,
}: Props) {
  const navigate = useNavigate();
  const { data: characters, isLoading } = useCharacterList(
    open ? projectId : undefined,
  );
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [keyword, setKeyword] = useState("");

  useEffect(() => {
    if (open) {
      setSelectedId(null);
      setKeyword("");
    }
  }, [open]);

  const filtered = (characters ?? []).filter((c) =>
    keyword ? c.name.toLowerCase().includes(keyword.toLowerCase()) : true,
  );

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogTitle>选择角色</DialogTitle>
        <div className="relative">
          <Search className="absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            placeholder="搜索角色..."
            value={keyword}
            onChange={(e) => setKeyword(e.target.value)}
            className="h-8 pl-8"
          />
        </div>
        <div className="max-h-[55vh] min-h-[200px] overflow-auto">
          {isLoading ? (
            <div className="flex h-[200px] items-center justify-center">
              <Loader2 className="size-6 animate-spin text-muted-foreground" />
            </div>
          ) : filtered.length === 0 ? (
            <div className="flex h-[200px] flex-col items-center justify-center gap-2 text-center">
              <User className="size-10 text-muted-foreground/30" />
              <p className="text-sm text-muted-foreground">还没有角色</p>
              {projectId && (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => {
                    onOpenChange(false);
                    navigate(`/project/${projectId}/subjects/character`);
                  }}
                >
                  前往角色库
                </Button>
              )}
            </div>
          ) : (
            <ul className="space-y-1">
              {filtered.map((c) => (
                <li key={c.id}>
                  <button
                    type="button"
                    onClick={() => setSelectedId(c.id)}
                    onDoubleClick={() => onSelect(c.id)}
                    className={cn(
                      "flex w-full items-center gap-3 rounded-md border px-3 py-2 text-left text-sm transition-colors hover:bg-accent",
                      selectedId === c.id &&
                        "border-primary ring-2 ring-primary/20",
                    )}
                  >
                    <User className="size-4 text-muted-foreground" />
                    <span className="font-medium">{c.name}</span>
                    <span className="flex-1 truncate text-muted-foreground">
                      {c.description || ""}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            取消
          </Button>
          <Button
            onClick={() => selectedId && onSelect(selectedId)}
            disabled={!selectedId}
          >
            确认选择
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
