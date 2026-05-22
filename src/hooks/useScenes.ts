import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type CreateSceneInput,
  type Scene,
  type UpdateSceneInput,
} from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export const sceneKeys = {
  all: (projectId: string) => ["scenes", projectId] as const,
  detail: (id: string) => ["scene", id] as const,
};

export function useSceneList(projectId: string | undefined) {
  return useQuery<Scene[]>({
    queryKey: projectId ? sceneKeys.all(projectId) : ["scenes", "none"],
    queryFn: () =>
      unwrap(commands.listScenes({ project_id: projectId as string })),
    enabled: !!projectId,
  });
}

export function useScene(id: string | undefined) {
  return useQuery<Scene>({
    queryKey: id ? sceneKeys.detail(id) : ["scene", "none"],
    queryFn: () => unwrap(commands.getScene(id as string)),
    enabled: !!id,
  });
}

export function useCreateScene() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateSceneInput) => unwrap(commands.createScene(input)),
    onSuccess: (_data, input) => {
      qc.invalidateQueries({ queryKey: sceneKeys.all(input.project_id) });
    },
  });
}

export function useUpdateScene(projectId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, input }: { id: string; input: UpdateSceneInput }) =>
      unwrap(commands.updateScene(id, input)),
    onSuccess: (_data, { id }) => {
      qc.invalidateQueries({ queryKey: sceneKeys.all(projectId) });
      qc.invalidateQueries({ queryKey: sceneKeys.detail(id) });
    },
  });
}

export function useDeleteScene(projectId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.deleteScene(id)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: sceneKeys.all(projectId) });
    },
  });
}
