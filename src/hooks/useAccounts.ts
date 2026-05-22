import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type ApiAccount,
  type CreateApiAccountInput,
  type UpdateApiAccountInput_Deserialize,
} from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export const accountKeys = {
  all: (providerId?: string) =>
    providerId ? (["accounts", providerId] as const) : (["accounts"] as const),
  detail: (id: string) => ["account", id] as const,
};

export function useAccountList(providerId?: string) {
  return useQuery<ApiAccount[]>({
    queryKey: accountKeys.all(providerId),
    queryFn: () => unwrap(commands.listApiAccounts(providerId ?? null)),
  });
}

export function useAccount(id: string | undefined) {
  return useQuery<ApiAccount>({
    queryKey: id ? accountKeys.detail(id) : ["account", "none"],
    queryFn: () => unwrap(commands.getApiAccount(id as string)),
    enabled: !!id,
  });
}

export function useCreateAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateApiAccountInput) =>
      unwrap(commands.createApiAccount(input)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["accounts"] });
    },
  });
}

export function useUpdateAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (vars: {
      id: string;
      input: UpdateApiAccountInput_Deserialize;
    }) => unwrap(commands.updateApiAccount(vars.id, vars.input)),
    onSuccess: (_data, vars) => {
      qc.invalidateQueries({ queryKey: ["accounts"] });
      qc.invalidateQueries({ queryKey: accountKeys.detail(vars.id) });
    },
  });
}

export function useDeleteAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.deleteApiAccount(id)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["accounts"] });
    },
  });
}

/**
 * Local-only check that the keyring still holds a non-empty secret for the
 * given account. NOT a network test against the provider — that arrives in
 * MS2 under a separate IPC.
 */
export function useVerifyApiAccountStorage() {
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.verifyApiAccountStorage(id)),
  });
}
