import { useMemo } from "react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useProviderList } from "@/hooks/useProviders";
import { useAccountList } from "@/hooks/useAccounts";
import type { TaskKind } from "@/lib/bindings/commands";

export interface ModelChoice {
  providerId: string;
  modelId: string;
  taskType: TaskKind;
  accountId: string;
}

interface ModelPickerProps {
  value: ModelChoice | null;
  onChange: (next: ModelChoice | null) => void;
}

// MS2: provider-model mapping hardcoded. Unified model registry arrives in MS3.
const MODEL_CATALOG: Array<{
  providerId: string;
  modelId: string;
  label: string;
  taskType: TaskKind;
}> = [
  {
    providerId: "bailian",
    modelId: "wan2.7-image-pro",
    label: "通义万相 2.7 Pro（文生图）",
    taskType: "image",
  },
  {
    providerId: "bailian",
    modelId: "happyhorse-1.0-r2v",
    label: "快乐马 1.0（参考图生视频）",
    taskType: "video",
  },
];

export function ModelPicker({ value, onChange }: ModelPickerProps) {
  const providers = useProviderList();
  const accounts = useAccountList();

  // Filter out models whose provider has no configured account.
  const available = useMemo(() => {
    if (!providers.data || !accounts.data) return [];
    const accountByProvider = new Map<string, string>();
    for (const acct of accounts.data) {
      if (!accountByProvider.has(acct.provider_id)) {
        accountByProvider.set(acct.provider_id, acct.id);
      }
    }
    return MODEL_CATALOG.filter((m) => accountByProvider.has(m.providerId)).map(
      (m) => ({ ...m, accountId: accountByProvider.get(m.providerId)! }),
    );
  }, [providers.data, accounts.data]);

  if (!providers.isLoading && !accounts.isLoading && available.length === 0) {
    return (
      <div className="rounded-md border border-dashed p-4 text-sm text-muted-foreground">
        尚未配置任何 Provider 账号。请前往
        <a href="/settings/accounts" className="mx-1 underline">
          设置 → 账号管理
        </a>
        添加。
      </div>
    );
  }

  return (
    <Select
      value={value ? `${value.providerId}::${value.modelId}` : ""}
      onValueChange={(key) => {
        const [providerId, modelId] = key.split("::");
        const found = available.find(
          (m) => m.providerId === providerId && m.modelId === modelId,
        );
        if (found) onChange(found);
      }}
    >
      <SelectTrigger>
        <SelectValue placeholder="选择模型" />
      </SelectTrigger>
      <SelectContent>
        {available.map((m) => (
          <SelectItem
            key={`${m.providerId}::${m.modelId}`}
            value={`${m.providerId}::${m.modelId}`}
          >
            {m.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
