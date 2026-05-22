import { useQuery } from "@tanstack/react-query";
import { commands, type Model, type Provider } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export const providerKeys = {
  all: () => ["providers"] as const,
  models: (providerId?: string) =>
    providerId
      ? (["models", providerId] as const)
      : (["models", "all"] as const),
};

export function useProviderList() {
  return useQuery<Provider[]>({
    queryKey: providerKeys.all(),
    queryFn: () => unwrap(commands.listProviders()),
  });
}

export function useModelList(providerId?: string) {
  return useQuery<Model[]>({
    queryKey: providerKeys.models(providerId),
    queryFn: () => unwrap(commands.listModels(providerId ?? null)),
  });
}
