import { useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { Wand2, X } from "lucide-react";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { useTaskList } from "@/hooks/useTasks";
import { TaskCard } from "./TaskCard";
import { EmptyState } from "@/components/common/EmptyState";
import type { GenerationTask } from "@/lib/bindings/commands";
import { parseDbDate } from "@/lib/datetime";

type Filter = "all" | "running" | "failed";

interface TaskListPanelProps {
  projectId: string;
}

export function TaskListPanel({ projectId }: TaskListPanelProps) {
  const [filter, setFilter] = useState<Filter>("all");
  const tasks = useTaskList(projectId);
  const [searchParams, setSearchParams] = useSearchParams();
  const batchFilter = searchParams.get("batch");

  const filtered = useMemo(() => {
    if (!tasks.data) return [];
    let list: GenerationTask[] = tasks.data;
    if (batchFilter) {
      list = list.filter((t) => t.batch_id === batchFilter);
    }
    if (filter === "running") {
      list = list.filter(
        (t) => t.status === "pending" || t.status === "running",
      );
    } else if (filter === "failed") {
      list = list.filter((t) => t.status === "failed");
    }
    return [...list].sort(
      (a, b) =>
        parseDbDate(b.created_at).getTime() -
        parseDbDate(a.created_at).getTime(),
    );
  }, [tasks.data, filter, batchFilter]);

  const clearBatchFilter = () => {
    const next = new URLSearchParams(searchParams);
    next.delete("batch");
    setSearchParams(next, { replace: true });
  };

  return (
    <Card className="flex h-full flex-col">
      <Tabs
        value={filter}
        onValueChange={(v) => setFilter(v as Filter)}
        className="flex h-full flex-col"
      >
        <header className="flex items-center justify-between gap-3 border-b px-4 py-3">
          <div className="flex items-center gap-2">
            <h2 className="text-lg font-semibold">任务列表</h2>
            {batchFilter && (
              <span className="flex items-center gap-1 rounded-full bg-primary/10 px-2 py-0.5 text-xs text-primary">
                批次 {batchFilter.slice(0, 8)}
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="ml-0.5 h-4 w-4"
                  onClick={clearBatchFilter}
                  aria-label="清除批次筛选"
                >
                  <X className="h-3 w-3" />
                </Button>
              </span>
            )}
          </div>
          <TabsList>
            <TabsTrigger value="all">全部</TabsTrigger>
            <TabsTrigger value="running">进行中</TabsTrigger>
            <TabsTrigger value="failed">失败</TabsTrigger>
          </TabsList>
        </header>
        <TabsContent value={filter} className="flex-1 overflow-auto p-4">
          {filtered.length === 0 ? (
            <EmptyState
              icon={Wand2}
              title={filter === "all" ? "尚无任务" : "没有匹配的任务"}
              description={
                filter === "all" ? "在左侧表单提交首个生成任务。" : ""
              }
            />
          ) : (
            <div className="space-y-3">
              {filtered.map((t) => (
                <TaskCard key={t.id} task={t} projectId={projectId} />
              ))}
            </div>
          )}
        </TabsContent>
      </Tabs>
    </Card>
  );
}
