import { useMemo } from "react";
import { ErrorAlert } from "@/components/common/ErrorAlert";
import { Skeleton } from "@/components/ui/skeleton";
import { useAccountList } from "@/hooks/useAccounts";
import { useProviderList } from "@/hooks/useProviders";
import { ProviderGroup } from "./ProviderGroup";

export function AccountsPanel() {
  const providers = useProviderList();
  const accounts = useAccountList();

  const grouped = useMemo(() => {
    const map = new Map<string, typeof accounts.data>();
    for (const acct of accounts.data ?? []) {
      const list = map.get(acct.provider_id) ?? [];
      list.push(acct);
      map.set(acct.provider_id, list);
    }
    return map;
  }, [accounts.data]);

  if (providers.isLoading || accounts.isLoading) {
    return (
      <div className="space-y-6">
        <Skeleton className="h-32 w-full" />
        <Skeleton className="h-32 w-full" />
      </div>
    );
  }
  if (providers.isError) {
    return (
      <ErrorAlert
        title="无法加载 Provider 列表"
        error={providers.error}
        onRetry={() => providers.refetch()}
      />
    );
  }
  if (accounts.isError) {
    return (
      <ErrorAlert
        title="无法加载账号列表"
        error={accounts.error}
        onRetry={() => accounts.refetch()}
      />
    );
  }

  return (
    <div className="space-y-8">
      {providers.data?.map((provider) => (
        <ProviderGroup
          key={provider.id}
          provider={provider}
          accounts={grouped.get(provider.id) ?? []}
        />
      ))}
    </div>
  );
}
