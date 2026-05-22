import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type Costume,
  type CreateCostumeInput,
  type UpdateCostumeInput,
} from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export const costumeKeys = {
  all: (projectId: string) => ["costumes", projectId] as const,
  byCharacter: (projectId: string, characterId: string) =>
    ["costumes", projectId, "byCharacter", characterId] as const,
  detail: (id: string) => ["costume", id] as const,
};

export function useCostumeList(projectId: string | undefined) {
  return useQuery<Costume[]>({
    queryKey: projectId ? costumeKeys.all(projectId) : ["costumes", "none"],
    queryFn: () =>
      unwrap(commands.listCostumes({ project_id: projectId as string })),
    enabled: !!projectId,
  });
}

export function useCostumeListByCharacter(
  projectId: string | undefined,
  characterId: string | undefined,
) {
  return useQuery<Costume[]>({
    queryKey:
      projectId && characterId
        ? costumeKeys.byCharacter(projectId, characterId)
        : ["costumes", "none"],
    queryFn: () =>
      unwrap(
        commands.listCostumes({
          project_id: projectId as string,
          character_id: characterId as string,
        }),
      ),
    enabled: !!projectId && !!characterId,
  });
}

export function useCostume(id: string | undefined) {
  return useQuery<Costume>({
    queryKey: id ? costumeKeys.detail(id) : ["costume", "none"],
    queryFn: () => unwrap(commands.getCostume(id as string)),
    enabled: !!id,
  });
}

export function useCreateCostume() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateCostumeInput) =>
      unwrap(commands.createCostume(input)),
    onSuccess: (_data, input) => {
      // Invalidate any costume query under this project (covers the
      // by-character subkey as well thanks to prefix matching).
      qc.invalidateQueries({ queryKey: costumeKeys.all(input.project_id) });
    },
  });
}

export function useUpdateCostume(projectId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, input }: { id: string; input: UpdateCostumeInput }) =>
      unwrap(commands.updateCostume(id, input)),
    onSuccess: (_data, { id }) => {
      qc.invalidateQueries({ queryKey: costumeKeys.all(projectId) });
      qc.invalidateQueries({ queryKey: costumeKeys.detail(id) });
    },
  });
}

export function useDeleteCostume(projectId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.deleteCostume(id)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: costumeKeys.all(projectId) });
    },
  });
}
