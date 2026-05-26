import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  events,
  type CreateGenerationTaskInput,
  type GenerationTask,
  type GenerationTaskStatus,
} from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export const taskKeys = {
  all: () => ["tasks"] as const,
  list: (projectId?: string, status?: GenerationTaskStatus) =>
    ["tasks", "list", projectId ?? null, status ?? null] as const,
  detail: (id: string) => ["task", id] as const,
};

export function useTaskList(
  projectId?: string,
  status?: GenerationTaskStatus,
  limit?: number,
) {
  return useQuery<GenerationTask[]>({
    queryKey: taskKeys.list(projectId, status),
    queryFn: () =>
      unwrap(
        commands.listTasks(projectId ?? null, status ?? null, limit ?? null),
      ),
  });
}

export function useTaskById(id: string | undefined) {
  return useQuery<GenerationTask>({
    queryKey: id ? taskKeys.detail(id) : ["task", "none"],
    queryFn: () => unwrap(commands.getTask(id as string)),
    enabled: !!id,
  });
}

export function useSubmitTask() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateGenerationTaskInput) =>
      unwrap(commands.submitTask(input)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: taskKeys.all() });
    },
  });
}

export function useCancelTask() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (taskId: string) => unwrap(commands.cancelTask(taskId)),
    onSuccess: (_data, taskId) => {
      qc.invalidateQueries({ queryKey: taskKeys.all() });
      qc.invalidateQueries({ queryKey: taskKeys.detail(taskId) });
    },
  });
}

/**
 * Mounts a single global listener for `task-status-changed` events emitted by
 * the engine forwarder. On any event we invalidate the entire `tasks` subtree
 * — explicit cache patches will arrive in spec-18 once the UI surfaces task
 * panels with hot-path concerns.
 */
export function useTaskStatusListener() {
  const qc = useQueryClient();
  useEffect(() => {
    const unlisten = events.taskStatusChanged.listen((event) => {
      qc.invalidateQueries({ queryKey: taskKeys.all() });
      qc.invalidateQueries({
        queryKey: taskKeys.detail(event.payload.task_id),
      });
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [qc]);
}
