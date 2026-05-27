import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  events,
  type CreateGenerationTaskInput,
  type GenerationTask,
  type GenerationTaskEvent,
  type GenerationTaskStatus,
} from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";
import { toast } from "sonner";

export const taskKeys = {
  all: () => ["tasks"] as const,
  list: (projectId?: string, status?: GenerationTaskStatus) =>
    ["tasks", "list", projectId ?? null, status ?? null] as const,
  detail: (id: string) => ["task", id] as const,
};

export const taskEventKeys = {
  byTask: (taskId: string) => ["task", taskId, "events"] as const,
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
 * Retry a failed task by reading the original task params and submitting a new
 * task with the same configuration.
 */
export function useRetryTask() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (taskId: string) => {
      const task = await unwrap(commands.getTask(taskId));
      const params: CreateGenerationTaskInput = {
        project_id: task.project_id,
        shot_id: task.shot_id,
        provider_id: task.provider_id,
        model_id: task.model_id,
        account_id: task.account_id,
        task_type: task.task_type,
        params_json: task.params_json,
      };
      return unwrap(commands.submitTask(params));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: taskKeys.all() });
      toast.success("重试任务已提交");
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

export function useTaskEvents(taskId: string | undefined) {
  return useQuery<GenerationTaskEvent[]>({
    queryKey: taskId ? taskEventKeys.byTask(taskId) : ["task-events", "none"],
    queryFn: () => unwrap(commands.listTaskEvents(taskId as string)),
    enabled: !!taskId,
  });
}

/**
 * Mounts a single global listener for `task-event-logged` and
 * `task-progress-tick` events. On `EventLogged` we patch the cached event
 * timeline directly (avoids a roundtrip refetch). On `ProgressTick` we just
 * invalidate the task detail so the card re-renders with the latest hint —
 * progress is not stored, so there's nothing to patch.
 */
export function useTaskEventListener() {
  const qc = useQueryClient();
  useEffect(() => {
    const unlistenLogged = events.taskEventLogged.listen((e) => {
      const { task_id, event } = e.payload;
      qc.setQueryData<GenerationTaskEvent[]>(
        taskEventKeys.byTask(task_id),
        (prev) => (prev ? [...prev, event] : [event]),
      );
    });
    const unlistenProgress = events.taskProgressTick.listen((e) => {
      qc.invalidateQueries({
        queryKey: taskKeys.detail(e.payload.task_id),
      });
    });
    return () => {
      void unlistenLogged.then((fn) => fn());
      void unlistenProgress.then((fn) => fn());
    };
  }, [qc]);
}
