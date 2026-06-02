import { useEffect, useState, type FormEvent } from "react";
import { Wand2 } from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  useSetTaskMaxConcurrency,
  useTaskMaxConcurrency,
} from "@/hooks/useTaskMaxConcurrency";

const MIN = 1;
const MAX = 8;

/**
 * Settings card for the global task engine knobs (spec-27). Currently exposes
 * `task.max_concurrency` — the cap on how many provider tasks run at once.
 * Saving pushes a live semaphore swap on the backend, so new tasks honor the
 * new cap immediately; in-flight tasks finish on the old cap.
 */
export function GenerationSettingsSection() {
  const current = useTaskMaxConcurrency();
  const save = useSetTaskMaxConcurrency();
  const [draft, setDraft] = useState<string>("");

  useEffect(() => {
    if (current.data !== undefined) {
      setDraft(String(current.data));
    }
  }, [current.data]);

  const parsed = Number(draft);
  const isValid =
    draft !== "" && Number.isInteger(parsed) && parsed >= MIN && parsed <= MAX;
  const dirty = current.data !== undefined && parsed !== current.data;

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    if (!isValid) return;
    try {
      await save.mutateAsync(parsed);
      toast.success("已保存");
    } catch (err) {
      toast.error(
        `保存失败：${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <Wand2 className="size-4" />
          生成任务
        </CardTitle>
        <CardDescription>
          控制后端同时运行的生成任务数量。新设定即时影响后续提交的任务，正在跑的任务不会被中断。
        </CardDescription>
      </CardHeader>
      <CardContent>
        <form
          onSubmit={handleSubmit}
          className="flex flex-wrap items-end gap-3"
        >
          <div className="space-y-1.5">
            <Label htmlFor="task-concurrency">最大并发数（{MIN}–{MAX}）</Label>
            <Input
              id="task-concurrency"
              type="number"
              inputMode="numeric"
              min={MIN}
              max={MAX}
              step={1}
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              disabled={current.isLoading || save.isPending}
              className="w-28"
            />
          </div>
          <Button
            type="submit"
            disabled={!isValid || !dirty || save.isPending}
          >
            {save.isPending ? "保存中..." : "保存"}
          </Button>
        </form>
        {draft !== "" && !isValid && (
          <p className="mt-2 text-xs text-destructive">
            需在 {MIN} 到 {MAX} 之间的整数
          </p>
        )}
      </CardContent>
    </Card>
  );
}
