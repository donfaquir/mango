import { useMemo, useState } from "react";
import { Wand2 } from "lucide-react";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { Card } from "@/components/ui/card";
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

  const filtered = useMemo(() => {
    if (!tasks.data) return [];
    let list: GenerationTask[];
    if (filter === "running") {
      list = tasks.data.filter(
        (t) => t.status === "pending" || t.status === "running",
      );
    } else if (filter === "failed") {
      list = tasks.data.filter((t) => t.status === "failed");
    } else {
      list = tasks.data;
    }
    return [...list].sort(
      (a, b) =>
        parseDbDate(b.created_at).getTime() -
        parseDbDate(a.created_at).getTime(),
    );
  }, [tasks.data, filter]);

  return (
    <Card className="flex h-full flex-col">
      <Tabs
        value={filter}
        onValueChange={(v) => setFilter(v as Filter)}
        className="flex h-full flex-col"
      >
        <header className="flex items-center justify-between border-b px-4 py-3">
          <h2 className="text-lg font-semibold">任务列表</h2>
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
