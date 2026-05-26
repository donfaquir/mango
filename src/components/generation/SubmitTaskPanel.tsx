import { useState } from "react";
import { toast } from "sonner";
import { Card } from "@/components/ui/card";
import { ModelPicker, type ModelChoice } from "./ModelPicker";
import { Wan27Params } from "./params/Wan27Params";
import { HappyhorseParams } from "./params/HappyhorseParams";
import { useSubmitTask } from "@/hooks/useTasks";
import type { Wan27FormValues } from "./params/types";
import type { HappyhorseFormValues } from "./params/types";

interface SubmitTaskPanelProps {
  projectId: string;
}

export function SubmitTaskPanel({ projectId }: SubmitTaskPanelProps) {
  const [model, setModel] = useState<ModelChoice | null>(null);
  const submit = useSubmitTask();

  const handleWan27Submit = async (values: Wan27FormValues) => {
    if (!model) return;
    const paramsJson = JSON.stringify({
      prompt: values.prompt,
      n: values.n,
      size: values.size,
      enable_sequential: values.enable_sequential,
      negative_prompt: values.negative_prompt ?? null,
    });
    try {
      await submit.mutateAsync({
        provider_id: model.providerId,
        model_id: model.modelId,
        account_id: model.accountId,
        task_type: model.taskType,
        params_json: paramsJson,
      });
      toast.success("任务已提交，可在右侧面板查看进度");
    } catch (err) {
      toast.error(
        `提交失败：${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  const handleHappyhorseSubmit = async (values: HappyhorseFormValues) => {
    if (!model) return;
    const paramsJson = JSON.stringify({
      prompt: values.prompt,
      media: values.subject_ids.map((id) => ({
        asset_id: id,
        type: "reference_image",
      })),
      resolution: values.resolution,
      ratio: values.ratio,
      duration: values.duration,
    });
    try {
      await submit.mutateAsync({
        provider_id: model.providerId,
        model_id: model.modelId,
        account_id: model.accountId,
        task_type: model.taskType,
        params_json: paramsJson,
      });
      toast.success("任务已提交，可在右侧面板查看进度");
    } catch (err) {
      toast.error(
        `提交失败：${err instanceof Error ? err.message : String(err)}`,
      );
    }
  };

  return (
    <Card className="flex h-full flex-col overflow-auto p-4">
      <h2 className="mb-4 text-lg font-semibold">新建生成任务</h2>

      <div className="space-y-4">
        <div className="space-y-1.5">
          <label className="text-sm font-medium">
            模型 <span className="text-destructive">*</span>
          </label>
          <ModelPicker value={model} onChange={setModel} />
        </div>

        {model?.modelId === "wan2.7-image-pro" && (
          <Wan27Params
            submitting={submit.isPending}
            onSubmit={handleWan27Submit}
          />
        )}

        {model?.modelId === "happyhorse-1.0-r2v" && (
          <HappyhorseParams
            projectId={projectId}
            submitting={submit.isPending}
            onSubmit={handleHappyhorseSubmit}
          />
        )}

        {!model && (
          <p className="mt-6 text-sm text-muted-foreground">请先选择模型</p>
        )}
      </div>
    </Card>
  );
}
