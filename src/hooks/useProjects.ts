import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type CreateProjectInput,
  type Project,
  type UpdateProjectInput,
} from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export const projectKeys = {
  all: ["projects"] as const,
  detail: (id: string) => ["projects", id] as const,
};

export function useProjectList() {
  return useQuery<Project[]>({
    queryKey: projectKeys.all,
    queryFn: () => unwrap(commands.listProjects(null)),
  });
}

export function useProject(id: string | undefined) {
  return useQuery<Project>({
    queryKey: id ? projectKeys.detail(id) : projectKeys.all,
    queryFn: () => unwrap(commands.getProject(id as string)),
    enabled: !!id,
  });
}

export function useCreateProject() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateProjectInput) =>
      unwrap(commands.createProject(input)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: projectKeys.all });
    },
  });
}

export function useUpdateProject() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, input }: { id: string; input: UpdateProjectInput }) =>
      unwrap(commands.updateProject(id, input)),
    onSuccess: (_data, { id }) => {
      qc.invalidateQueries({ queryKey: projectKeys.all });
      qc.invalidateQueries({ queryKey: projectKeys.detail(id) });
    },
  });
}

export function useDeleteProject() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.deleteProject(id)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: projectKeys.all });
    },
  });
}
