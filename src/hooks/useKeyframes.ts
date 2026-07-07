import { useQuery } from "@tanstack/react-query";
import { commands } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export function useKeyframes(itemId: string | undefined) {
  return useQuery({
    queryKey: ["keyframes", itemId],
    queryFn: () => unwrap(commands.listKeyframes(itemId!)),
    enabled: !!itemId,
  });
}

export function useKenBurnsPresets() {
  return useQuery({
    queryKey: ["kenBurnsPresets"],
    queryFn: () => unwrap(commands.listKenBurnsPresets()),
    staleTime: Infinity,
  });
}

export function useTransitionPresets() {
  return useQuery({
    queryKey: ["transitionPresets"],
    queryFn: () => unwrap(commands.listTransitionPresets()),
    staleTime: Infinity,
  });
}
