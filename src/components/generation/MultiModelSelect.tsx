import { useMemo } from "react";
import { CheckIcon, ImageIcon, VideoIcon } from "lucide-react";

import { useModelList } from "@/hooks/useProviders";
import type { Model, TaskKind } from "@/lib/bindings/commands";
import { cn } from "@/lib/utils";

export interface MultiModelSelectProps {
  /// Only models whose model_type matches are shown. Batches mix exactly one
  /// task_type so cross-type selection isn't a real workflow.
  taskType: TaskKind;
  /// When set, restrict the picker to one provider (MS4 ships with bailian
  /// only — kept as a knob for forward-compat).
  providerId?: string;
  /// Selected model IDs. Order preserved from click sequence.
  value: string[];
  onChange: (next: string[]) => void;
  /// Optional empty-state copy override for tight contexts.
  emptyHint?: string;
}

export function MultiModelSelect({
  taskType,
  providerId,
  value,
  onChange,
  emptyHint,
}: MultiModelSelectProps) {
  const models = useModelList(providerId);

  const filtered = useMemo(() => {
    if (!models.data) return [];
    return models.data.filter((m) => m.model_type === taskType);
  }, [models.data, taskType]);

  const selected = useMemo(() => new Set(value), [value]);

  const toggle = (modelId: string) => {
    if (selected.has(modelId)) {
      onChange(value.filter((id) => id !== modelId));
    } else {
      onChange([...value, modelId]);
    }
  };

  if (models.isLoading) {
    return (
      <div className="text-sm text-muted-foreground">加载模型列表…</div>
    );
  }

  if (filtered.length === 0) {
    return (
      <div className="rounded-md border border-dashed p-3 text-sm text-muted-foreground">
        {emptyHint ?? `没有可用于 ${taskTypeLabel(taskType)} 的模型`}
      </div>
    );
  }

  return (
    <div role="group" aria-label="模型多选" className="space-y-1.5">
      {filtered.map((m) => {
        const isChecked = selected.has(m.id);
        return (
          <button
            type="button"
            key={m.id}
            role="checkbox"
            aria-checked={isChecked}
            onClick={() => toggle(m.id)}
            className={cn(
              "flex w-full items-center gap-3 rounded-md border px-3 py-2 text-left text-sm transition",
              "hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
              isChecked
                ? "border-primary bg-primary/5"
                : "border-input bg-background",
            )}
          >
            <span
              className={cn(
                "flex h-4 w-4 shrink-0 items-center justify-center rounded-sm border",
                isChecked
                  ? "border-primary bg-primary text-primary-foreground"
                  : "border-input",
              )}
            >
              {isChecked && <CheckIcon className="h-3 w-3" />}
            </span>
            <span className="flex-1 truncate font-medium">{m.name}</span>
            <ModelBadges model={m} />
          </button>
        );
      })}
    </div>
  );
}

function ModelBadges({ model }: { model: Model }) {
  const cap = tryParseCapabilities(model.capabilities_json);
  const requiresRef = cap?.requires_reference_media === true;
  const Icon = model.model_type === "video" ? VideoIcon : ImageIcon;
  return (
    <span className="flex shrink-0 items-center gap-1.5 text-xs text-muted-foreground">
      <Icon className="h-3.5 w-3.5" />
      {requiresRef && (
        <span className="rounded-sm bg-amber-100 px-1.5 py-0.5 text-amber-900 dark:bg-amber-950 dark:text-amber-200">
          需参考图
        </span>
      )}
    </span>
  );
}

function tryParseCapabilities(
  raw: string | null,
): Record<string, unknown> | null {
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw);
    return typeof parsed === "object" && parsed ? parsed : null;
  } catch {
    return null;
  }
}

function taskTypeLabel(t: TaskKind): string {
  switch (t) {
    case "image":
      return "图像";
    case "video":
      return "视频";
    case "audio":
      return "音频";
    case "text":
      return "文本";
  }
}
