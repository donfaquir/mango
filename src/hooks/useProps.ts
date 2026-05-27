import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type CreatePropInput,
  type Prop,
  type UpdatePropInput,
} from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export const propKeys = {
  all: (projectId: string) => ["props", projectId] as const,
  detail: (id: string) => ["prop", id] as const,
};

export function usePropList(projectId: string | undefined) {
  return useQuery<Prop[]>({
    queryKey: projectId ? propKeys.all(projectId) : ["props", "none"],
    queryFn: () =>
      unwrap(commands.listProps({ project_id: projectId as string })),
    enabled: !!projectId,
  });
}

export function useProp(id: string | undefined) {
  return useQuery<Prop>({
    queryKey: id ? propKeys.detail(id) : ["prop", "none"],
    queryFn: () => unwrap(commands.getProp(id as string)),
    enabled: !!id,
  });
}

export function useCreateProp() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreatePropInput) => unwrap(commands.createProp(input)),
    onSuccess: (_data, input) => {
      qc.invalidateQueries({ queryKey: propKeys.all(input.project_id) });
    },
  });
}

export function useUpdateProp(projectId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, input }: { id: string; input: UpdatePropInput }) =>
      unwrap(commands.updateProp(id, input)),
    onSuccess: (_data, { id }) => {
      qc.invalidateQueries({ queryKey: propKeys.all(projectId) });
      qc.invalidateQueries({ queryKey: propKeys.detail(id) });
    },
  });
}

export function useDeleteProp(projectId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.deleteProp(id)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: propKeys.all(projectId) });
    },
  });
}
