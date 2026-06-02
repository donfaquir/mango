import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { Film, Loader2, Search } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useShotList } from "@/hooks/useShots";
import { cn } from "@/lib/utils";

interface Props {
  open: boolean;
  episodeId: string | undefined;
  onOpenChange: (open: boolean) => void;
  onSelect: (shotId: string) => void;
}

export function ShotPickerDialog({
  open,
  episodeId,
  onOpenChange,
  onSelect,
}: Props) {
  const { projectId } = useParams<{ projectId: string }>();
  const navigate = useNavigate();
  const { data: shots, isLoading } = useShotList(open ? episodeId : undefined);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [keyword, setKeyword] = useState("");

  useEffect(() => {
    if (open) {
      setSelectedId(null);
      setKeyword("");
    }
  }, [open]);

  const filtered = (shots ?? []).filter((s) =>
    keyword ? s.summary.toLowerCase().includes(keyword.toLowerCase()) : true,
  );

  const handleConfirm = () => {
    if (selectedId) {
      onSelect(selectedId);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogTitle>选择分镜</DialogTitle>
        <div className="relative">
          <Search className="absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            placeholder="搜索摘要..."
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
              <Film className="size-10 text-muted-foreground/30" />
              <p className="text-sm text-muted-foreground">该剧集还没有分镜</p>
              {projectId && episodeId && (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => {
                    onOpenChange(false);
                    navigate(`/project/${projectId}/episodes/${episodeId}`);
                  }}
                >
                  跳转剧集详情
                </Button>
              )}
            </div>
          ) : (
            <ul className="space-y-1">
              {filtered.map((shot) => (
                <li key={shot.id}>
                  <button
                    type="button"
                    onClick={() => setSelectedId(shot.id)}
                    onDoubleClick={() => onSelect(shot.id)}
                    className={cn(
                      "flex w-full items-center gap-3 rounded-md border px-3 py-2 text-left text-sm transition-colors hover:bg-accent",
                      selectedId === shot.id &&
                        "border-primary ring-2 ring-primary/20",
                    )}
                  >
                    <Film className="size-4 text-muted-foreground" />
                    <span className="font-medium">#{shot.order_index}</span>
                    <span className="flex-1 truncate text-muted-foreground">
                      {shot.summary || "（无摘要）"}
                    </span>
                    <span className="text-[11px] text-muted-foreground">
                      {shot.status}
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
          <Button onClick={handleConfirm} disabled={!selectedId}>
            确认选择
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
