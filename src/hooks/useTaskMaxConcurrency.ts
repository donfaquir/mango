import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export const taskMaxConcurrencyKeys = {
  current: () => ["task", "max_concurrency"] as const,
};

export function useTaskMaxConcurrency() {
  return useQuery<number>({
    queryKey: taskMaxConcurrencyKeys.current(),
    queryFn: () => unwrap(commands.getTaskMaxConcurrency()),
  });
}

export function useSetTaskMaxConcurrency() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (value: number) =>
      unwrap(commands.setTaskMaxConcurrency(value)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: taskMaxConcurrencyKeys.current() });
    },
  });
}
