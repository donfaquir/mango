import { useEffect, useMemo } from "react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useModelList, useProviderList } from "@/hooks/useProviders";
import { useAccountList } from "@/hooks/useAccounts";
import type { ApiAccount, TaskKind, Model } from "@/lib/bindings/commands";

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

export function ModelPicker({ value, onChange }: ModelPickerProps) {
  const providers = useProviderList();
  const models = useModelList();
  const accounts = useAccountList();

  const accountsByProvider = useMemo(() => {
    const m = new Map<string, ApiAccount[]>();
    if (!accounts.data) return m;
    for (const a of accounts.data) {
      const list = m.get(a.provider_id) ?? [];
      list.push(a);
      m.set(a.provider_id, list);
    }
    return m;
  }, [accounts.data]);

  const available = useMemo(() => {
    if (!providers.data || !accounts.data || !models.data) return [];
    return models.data
      .filter(isTaskModel)
      .filter((m) => accountsByProvider.has(m.provider_id))
      .map((m) => ({
        providerId: m.provider_id,
        modelId: m.id,
        label: modelLabelWithCapability(m),
        taskType: m.model_type,
      }));
  }, [providers.data, accounts.data, models.data, accountsByProvider]);

  const selectedModel = value
    ? available.find(
        (m) => m.providerId === value.providerId && m.modelId === value.modelId,
      ) ?? null
    : null;
  const accountsForSelected = selectedModel
    ? accountsByProvider.get(selectedModel.providerId) ?? []
    : [];

  // Re-anchor accountId when the parent's value points at an account that is
  // no longer under the selected provider — happens when the picked account
  // was deleted, or when the model switched providers.
  useEffect(() => {
    if (!selectedModel || accountsForSelected.length === 0) return;
    const stillValid = accountsForSelected.some(
      (a) => a.id === value?.accountId,
    );
    if (!stillValid) {
      onChange({
        providerId: selectedModel.providerId,
        modelId: selectedModel.modelId,
        taskType: selectedModel.taskType,
        accountId: accountsForSelected[0].id,
      });
    }
  }, [selectedModel, accountsForSelected, value?.accountId, onChange]);

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

  const handleModelChange = (key: string) => {
    const [providerId, modelId] = key.split("::");
    const found = available.find(
      (m) => m.providerId === providerId && m.modelId === modelId,
    );
    if (!found) return;
    const accts = accountsByProvider.get(found.providerId) ?? [];
    if (accts.length === 0) return;
    // Prefer keeping the user's current accountId if it still matches the new
    // model's provider — switching between two models of the same provider
    // shouldn't silently change which account gets billed.
    const reusable = accts.find((a) => a.id === value?.accountId);
    onChange({
      providerId: found.providerId,
      modelId: found.modelId,
      taskType: found.taskType,
      accountId: reusable?.id ?? accts[0].id,
    });
  };

  const handleAccountChange = (accountId: string) => {
    if (!value) return;
    onChange({ ...value, accountId });
  };

  return (
    <div className="space-y-3">
      <Select
        value={value ? `${value.providerId}::${value.modelId}` : ""}
        onValueChange={handleModelChange}
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

      {accountsForSelected.length > 1 && value && (
        <div className="space-y-1.5">
          <label className="text-sm font-medium">使用账号</label>
          <Select value={value.accountId} onValueChange={handleAccountChange}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {accountsForSelected.map((a) => (
                <SelectItem key={a.id} value={a.id}>
                  {formatAccountOption(a)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      )}
    </div>
  );
}

function formatAccountOption(a: ApiAccount): string {
  const last4 = a.key_last4 ? ` · ····${a.key_last4}` : "";
  return `${a.label}${last4}`;
}

function isTaskKind(v: string): v is TaskKind {
  return v === "text" || v === "image" || v === "video" || v === "audio";
}

function isTaskModel(model: Model): model is Model & { model_type: TaskKind } {
  return isTaskKind(model.model_type);
}

function modelLabelWithCapability(model: Model): string {
  const base = model.name;
  const cap = tryParseCapabilities(model.capabilities_json);
  if (!cap) return base;
  const tags: string[] = [];
  if (cap.requires_reference_media === true) {
    tags.push("需参考图");
  }
  if (Array.isArray(cap.task_types) && cap.task_types.length > 0) {
    tags.push(cap.task_types.join("/"));
  }
  return tags.length > 0 ? `${base}（${tags.join(" · ")}）` : base;
}

function tryParseCapabilities(raw: string | null): Record<string, unknown> | null {
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw);
    return typeof parsed === "object" && parsed ? parsed : null;
  } catch {
    return null;
  }
}
