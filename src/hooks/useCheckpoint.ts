import { useMutation } from "@tanstack/react-query";
import { commands, type CreateCheckpointInput } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export function useCreateCheckpoint() {
  return useMutation({
    mutationFn: (input: CreateCheckpointInput) =>
      unwrap(commands.createEpisodeCheckpoint(input)),
  });
}
