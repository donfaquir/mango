import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useAccountList } from "@/hooks/useAccounts";
import { useCharacterList } from "@/hooks/useCharacters";
import { useModelList } from "@/hooks/useProviders";
import { useShotList } from "@/hooks/useShots";
import { useSubmitTasksBatch } from "@/hooks/useTasks";
import type { TaskKind } from "@/lib/bindings/commands";

import { MultiModelSelect } from "./MultiModelSelect";
import {
  buildBatchTaskInputs,
  type SkippedItem,
} from "./buildBatchTaskInputs";

// Group skipped shots by reason so the user sees "缺少图像 prompt: #1、#2"
// instead of an opaque list of shot labels.
function summarizeSkipped(skipped: SkippedItem[]): string {
  const byReason = new Map<string, string[]>();
  for (const s of skipped) {
    const list = byReason.get(s.reason) ?? [];
    list.push(s.shotLabel);
    byReason.set(s.reason, list);
  }
  return Array.from(byReason.entries())
    .map(([reason, labels]) => `${reason}（${labels.join("、")}）`)
    .join("；");
}

const PROVIDER_ID = "bailian";

interface BatchSubmitDialogProps {
  projectId: string;
  episodeId: string;
  selectedShotIds: string[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSubmitted?: (batchId: string) => void;
}

export function BatchSubmitDialog({
  projectId,
  episodeId,
  selectedShotIds,
  open,
  onOpenChange,
  onSubmitted,
}: BatchSubmitDialogProps) {
  const [taskType, setTaskType] = useState<TaskKind>("image");
  const [modelIds, setModelIds] = useState<string[]>([]);
  const [accountId, setAccountId] = useState<string>("");

  const shots = useShotList(episodeId);
  const models = useModelList(PROVIDER_ID);
  const accounts = useAccountList(PROVIDER_ID);
  const characters = useCharacterList(projectId);
  const submit = useSubmitTasksBatch();

  const selectedShots = useMemo(() => {
    if (!shots.data) return [];
    const sel = new Set(selectedShotIds);
    return shots.data.filter((s) => sel.has(s.id));
  }, [shots.data, selectedShotIds]);

  const selectedModels = useMemo(() => {
    if (!models.data) return [];
    const sel = new Set(modelIds);
    return models.data.filter((m) => sel.has(m.id));
  }, [models.data, modelIds]);

  const plannedCount = selectedShots.length * selectedModels.length;

  // Auto-pick the only account when the user hasn't chosen one. Done in an
  // effect so we don't trigger setState during render.
  useEffect(() => {
    if (accounts.data && accounts.data.length === 1 && !accountId) {
      setAccountId(accounts.data[0].id);
    }
  }, [accounts.data, accountId]);

  const handleSubmit = async () => {
    if (!accountId || selectedModels.length === 0) return;
    const { inputs, skipped } = await buildBatchTaskInputs({
      projectId,
      accountId,
      providerId: PROVIDER_ID,
      taskType,
      shots: selectedShots,
      models: selectedModels,
      characters: characters.data ?? [],
    });

    if (inputs.length === 0) {
      const detail =
        skipped.length === 0
          ? "未选中任何分镜"
          : summarizeSkipped(skipped);
      toast.error(`所有分镜都被跳过：${detail}`);
      return;
    }

    try {
      const outcome = await submit.mutateAsync(inputs);
      if (skipped.length > 0) {
        toast.warning(`已跳过 ${skipped.length} 个分镜：${summarizeSkipped(skipped)}`);
      }
      toast.success(`已提交 ${outcome.created_count} 个任务`);
      onSubmitted?.(outcome.batch_id);
      onOpenChange(false);
    } catch (err) {
      toast.error(
        `批量提交失败：${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  const canSubmit =
    accountId !== "" && selectedModels.length > 0 && plannedCount > 0;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>批量生成任务</DialogTitle>
          <DialogDescription>
            选中 {selectedShotIds.length} 个分镜，挑选模型后将按笛卡尔积创建任务。
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <div className="space-y-1.5">
            <label className="text-sm font-medium">任务类型</label>
            <Select
              value={taskType}
              onValueChange={(v) => {
                setTaskType(v as TaskKind);
                setModelIds([]);
              }}
            >
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="image">图像</SelectItem>
                <SelectItem value="video">视频</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-1.5">
            <label className="text-sm font-medium">模型（可多选）</label>
            <MultiModelSelect
              taskType={taskType}
              providerId={PROVIDER_ID}
              value={modelIds}
              onChange={setModelIds}
            />
          </div>

          {accounts.data && accounts.data.length > 1 && (
            <div className="space-y-1.5">
              <label className="text-sm font-medium">使用账号</label>
              <Select value={accountId} onValueChange={setAccountId}>
                <SelectTrigger>
                  <SelectValue placeholder="选择账号" />
                </SelectTrigger>
                <SelectContent>
                  {accounts.data.map((a) => (
                    <SelectItem key={a.id} value={a.id}>
                      {a.label}
                      {a.key_last4 ? ` · ····${a.key_last4}` : ""}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          )}

          {accounts.data && accounts.data.length === 0 && (
            <p className="rounded-md border border-dashed p-3 text-sm text-muted-foreground">
              尚未配置百炼账号，请前往设置 → 账号管理添加。
            </p>
          )}

          <div className="rounded-md border bg-muted/40 p-3 text-sm">
            将创建 <strong>{plannedCount}</strong> 个任务（{selectedShots.length}{" "}
            分镜 × {selectedModels.length} 模型）
          </div>
        </div>

        <DialogFooter>
          <Button
            type="button"
            variant="ghost"
            onClick={() => onOpenChange(false)}
            disabled={submit.isPending}
          >
            取消
          </Button>
          <Button
            type="button"
            onClick={handleSubmit}
            disabled={!canSubmit || submit.isPending}
          >
            {submit.isPending ? "提交中…" : `提交 ${plannedCount} 个任务`}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
