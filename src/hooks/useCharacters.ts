import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type Character,
  type CreateCharacterInput,
  type UpdateCharacterInput,
} from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export const characterKeys = {
  all: (projectId: string) => ["characters", projectId] as const,
  detail: (id: string) => ["character", id] as const,
};

export function useCharacterList(projectId: string | undefined) {
  return useQuery<Character[]>({
    queryKey: projectId ? characterKeys.all(projectId) : ["characters", "none"],
    queryFn: () =>
      unwrap(commands.listCharacters({ project_id: projectId as string })),
    enabled: !!projectId,
  });
}

export function useCharacter(id: string | undefined) {
  return useQuery<Character>({
    queryKey: id ? characterKeys.detail(id) : ["character", "none"],
    queryFn: () => unwrap(commands.getCharacter(id as string)),
    enabled: !!id,
  });
}

export function useCreateCharacter() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateCharacterInput) =>
      unwrap(commands.createCharacter(input)),
    onSuccess: (_data, input) => {
      qc.invalidateQueries({ queryKey: characterKeys.all(input.project_id) });
    },
  });
}

export function useUpdateCharacter(projectId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, input }: { id: string; input: UpdateCharacterInput }) =>
      unwrap(commands.updateCharacter(id, input)),
    onSuccess: (_data, { id }) => {
      qc.invalidateQueries({ queryKey: characterKeys.all(projectId) });
      qc.invalidateQueries({ queryKey: characterKeys.detail(id) });
    },
  });
}

export function useDeleteCharacter(projectId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.deleteCharacter(id)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: characterKeys.all(projectId) });
      // Costumes cascade-delete in DB; invalidate their list for this project.
      qc.invalidateQueries({ queryKey: ["costumes", projectId] });
    },
  });
}
