# Spec 18 — 任务 UI（提交 / 列表 / 预览）

> 对应 MS2 任务 6/7/8。在已有项目工作区下加一个「AI 生成」面板，承担三件事：
> 1. **提交表单**：选模型 → 填写参数 → 提交（动态字段，wan2.7 vs happyhorse 参数面不同；happyhorse 需主体选择器）
> 2. **任务面板**：实时列表，按时间倒序展示状态/进度/错误/重试
> 3. **结果预览**：图片缩略图点开放大；视频缩略图点开播放

依赖：spec-15（前端 hook + bindings）+ spec-16（账号/Provider 列表）+ spec-17（百炼 provider 已注册可被列出）

非目标（本 spec 不做）：「采用结果绑定到分镜」（MS3 才有分镜概念）、批量提交（MS4）、图片局部编辑（MS6）。

---

## 1. 路由与入口

### 1.1 新增路由

`src/routes.tsx`：在 `project/:projectId` 子路由树内追加

```tsx
{
  path: "generation",
  element: <GenerationWorkspacePage />,
  children: [
    { index: true, element: <Navigate to="new" replace /> },
    { path: "new", element: <SubmitTaskPanel /> },
    { path: "tasks", element: <TaskListPanel /> },
  ],
},
```

### 1.2 Workspace 内导航

`src/components/layout/` 已有项目级导航条（subjects 用过）。把它抽出成 `WorkspaceNav.tsx`（如果还没抽），加一个 tab：「AI 生成」→ `/project/:id/generation`。

> **不要**为「new / tasks」做 sub-tab 的强切换；表单（new）和列表（tasks）放同屏左右两栏，切换只是滚动焦点：左 1/3 表单，右 2/3 列表。route 上 `/new` 和 `/tasks` 的区别只用于 deep-link，UI 始终双栏。

```
┌─────────────────────────────────────────────────────────────────┐
│ 项目顶栏（项目名 / 标签页：主体库 / AI 生成 / ...）              │
├──────────────────────────┬──────────────────────────────────────┤
│ 提交表单 (SubmitTask)    │ 任务面板 (TaskList)                  │
│  - Provider 选择          │  - 卡片列表（最新在上）              │
│  - 模型选择               │  - 状态徽章 / 进度条                  │
│  - 动态参数面板           │  - 缩略图（成功后展示）               │
│  - Prompt textarea        │  - 失败错误 + 重试                    │
│  - 主体选择（仅视频）     │  - 点开 → 大图弹窗 / 视频播放          │
│  - 提交按钮               │                                      │
└──────────────────────────┴──────────────────────────────────────┘
```

### 1.3 页面外壳

`src/pages/GenerationWorkspacePage.tsx`（≤ 60 行）：纯布局，不含数据逻辑。

```tsx
import { useParams } from "react-router-dom";
import { SubmitTaskPanel } from "@/components/generation/SubmitTaskPanel";
import { TaskListPanel } from "@/components/generation/TaskListPanel";
import { useTaskStatusListener } from "@/hooks/useTasks";

export default function GenerationWorkspacePage() {
  const { projectId } = useParams<{ projectId: string }>();
  // Single global listener → invalidates TanStack Query caches.
  // Mounting it at this page level keeps listener lifecycle tied to the workspace.
  useTaskStatusListener(projectId);

  if (!projectId) return null;

  return (
    <div className="grid h-full grid-cols-[minmax(360px,1fr)_2fr] gap-6 p-6">
      <SubmitTaskPanel projectId={projectId} />
      <TaskListPanel projectId={projectId} />
    </div>
  );
}
```

---

## 2. 文件清单（新建）

```
src/components/generation/
├── SubmitTaskPanel.tsx              # 表单外壳（管 model 选择，分发给参数子表单）
├── ModelPicker.tsx                  # provider→model 二级选择器，过滤掉无账号的 provider
├── PromptField.tsx                  # 多行 prompt + 字符计数
├── SubjectPickerDialog.tsx          # 主体选择弹窗（list + 缩略图 + 选中）
├── ReferenceImagePreview.tsx        # 选中主体后的参考图缩略展示行
├── params/
│   ├── Wan27Params.tsx              # n / size / enable_sequential / negative_prompt
│   ├── HappyhorseParams.tsx        # resolution / ratio / duration + 主体选择
│   └── types.ts                     # Wan27FormValues / HappyhorseFormValues
├── TaskListPanel.tsx                # 列表外壳 + 状态过滤（全部/进行中/失败）
├── TaskCard.tsx                     # 单任务卡片：缩略图 + 状态 + 元数据 + actions
├── TaskStatusBadge.tsx              # pending/running/success/failed/cancelled 五种样式
├── TaskProgress.tsx                 # 0-100 进度条（running 时显示）
├── ResultPreviewDialog.tsx          # 图片放大 / 视频播放统一弹窗
└── useTaskFormDefaults.ts          # 给表单提供默认值（model_id 决定）

src/hooks/
└── useTasks.ts                      # listTasks / submitTask / cancelTask + listener
```

`useTasks.ts` 在 spec-15 已经定义过 hook 名（`useTaskList` / `useSubmitTask` / `useTaskStatusListener`）。spec-18 不重新定义，只引用并补三个：`useCancelTask`、`useTaskById`、`useRetryTask`（重试 = 用同样 params 提交一次新的 generation_task，不复用原 ID）。

---

## 3. 提交表单

### 3.1 字段规则

| 模型 | 必填 | 可选 |
|------|------|------|
| `wan2.7-image-pro` | prompt | n（1-4，默认 1）/ size（默认 `2K`）/ enable_sequential（默认 false）/ negative_prompt |
| `happyhorse-1.0-r2v` | prompt + 主体（≥1 个，且至少 1 张参考图）| resolution（720P/1080P）/ ratio（16:9/9:16/1:1）/ duration（3/5/10 秒，默认 5） |

### 3.2 数据流

```
ModelPicker
   │ value: { providerId, modelId }
   ▼
SubmitTaskPanel
   │ 根据 modelId 决定 render 哪个 params 子表单
   ▼
Wan27Params 或 HappyhorseParams
   │ 各自维护 form state（react-hook-form）
   ▼
useSubmitTask().mutateAsync({
  account_id,
  model_id,
  task_type: "image" | "video",
  prompt,
  provider_params: { ... },         // 模型私有参数（spec-17 schema）
})
   │ 成功 → toast + reset 表单 + invalidate task list
   ▼
任务卡片在右侧列表「Pending」位置出现
```

### 3.3 ModelPicker

```tsx
// src/components/generation/ModelPicker.tsx — 约 90 行
import { useMemo } from "react";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import { useProviderList } from "@/hooks/useProviders";
import { useAccountList } from "@/hooks/useAccounts";

export interface ModelChoice {
  providerId: string;
  modelId: string;
  taskType: "image" | "video";
  accountId: string;        // 选中后立刻锁定一个账号（取该 provider 第一个可用账号）
}

interface Props {
  value: ModelChoice | null;
  onChange: (next: ModelChoice | null) => void;
}

// MS2 阶段：provider 与 model 的对应关系硬编码在前端。
// 跨 provider 模型注册到 MS3 才统一。spec-17 注册的 wan2.7 / happyhorse 写死在这。
const MODEL_CATALOG: Array<{
  providerId: string;
  modelId: string;
  label: string;
  taskType: "image" | "video";
}> = [
  { providerId: "bailian", modelId: "wan2.7-image-pro", label: "通义万相 2.7 Pro（文生图）", taskType: "image" },
  { providerId: "bailian", modelId: "happyhorse-1.0-r2v", label: "快乐马 1.0（参考图生视频）", taskType: "video" },
];

export function ModelPicker({ value, onChange }: Props) {
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
    return MODEL_CATALOG
      .filter((m) => accountByProvider.has(m.providerId))
      .map((m) => ({ ...m, accountId: accountByProvider.get(m.providerId)! }));
  }, [providers.data, accounts.data]);

  if (!providers.isLoading && !accounts.isLoading && available.length === 0) {
    return (
      <div className="rounded-md border border-dashed p-4 text-sm text-muted-foreground">
        尚未配置任何 Provider 账号。请前往
        <a href="/settings/accounts" className="underline mx-1">设置 → 账号管理</a>
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
          <SelectItem key={`${m.providerId}::${m.modelId}`} value={`${m.providerId}::${m.modelId}`}>
            {m.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
```

### 3.4 Wan27Params

```tsx
// src/components/generation/params/Wan27Params.tsx — 约 110 行
import { useForm, Controller } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";

export const wan27FormSchema = z.object({
  prompt: z.string().min(1, "请输入 prompt").max(2000),
  negative_prompt: z.string().max(500).optional(),
  n: z.number().int().min(1).max(4).default(1),
  size: z.enum(["1K", "2K", "4K"]).default("2K"),
  enable_sequential: z.boolean().default(false),
});
export type Wan27FormValues = z.infer<typeof wan27FormSchema>;

interface Props {
  defaultValues?: Partial<Wan27FormValues>;
  onSubmit: (values: Wan27FormValues) => Promise<void> | void;
  submitting: boolean;
}

export function Wan27Params({ defaultValues, onSubmit, submitting }: Props) {
  const { register, handleSubmit, control, formState } = useForm<Wan27FormValues>({
    resolver: zodResolver(wan27FormSchema),
    defaultValues: { n: 1, size: "2K", enable_sequential: false, ...defaultValues },
  });

  return (
    <form onSubmit={handleSubmit(onSubmit)} className="space-y-4">
      <PromptField {...register("prompt")} error={formState.errors.prompt?.message} />

      <details className="rounded-md border bg-muted/30 px-3 py-2 text-sm">
        <summary className="cursor-pointer select-none">高级参数</summary>
        <div className="mt-3 space-y-3">
          <Field label="负面 prompt（可选）">
            <Textarea rows={2} {...register("negative_prompt")} />
          </Field>
          <Field label="生成张数 (n)">
            <Input type="number" min={1} max={4} {...register("n", { valueAsNumber: true })} />
          </Field>
          <Field label="分辨率">
            <Controller
              name="size"
              control={control}
              render={({ field }) => (
                <Select value={field.value} onValueChange={field.onChange}>
                  <SelectTrigger><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="1K">1K</SelectItem>
                    <SelectItem value="2K">2K</SelectItem>
                    <SelectItem value="4K">4K</SelectItem>
                  </SelectContent>
                </Select>
              )}
            />
          </Field>
          <Field label="组图保持一致 (enable_sequential)">
            <Controller
              name="enable_sequential"
              control={control}
              render={({ field }) => (
                <input
                  type="checkbox"
                  checked={field.value}
                  onChange={(e) => field.onChange(e.target.checked)}
                />
              )}
            />
          </Field>
        </div>
      </details>

      <Button type="submit" disabled={submitting}>
        {submitting ? "提交中..." : "提交生成"}
      </Button>
    </form>
  );
}
```

`Field` / `PromptField` 是同目录 helper（去重 label/error 模式）。

### 3.5 HappyhorseParams

```tsx
// src/components/generation/params/HappyhorseParams.tsx — 约 150 行
import { useForm, Controller } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { SubjectPickerDialog } from "../SubjectPickerDialog";
import { ReferenceImagePreview } from "../ReferenceImagePreview";

export const happyhorseFormSchema = z.object({
  prompt: z.string().min(1).max(2000),
  subject_ids: z.array(z.string().uuid()).min(1, "至少选择一个主体"),
  resolution: z.enum(["720P", "1080P"]).default("720P"),
  ratio: z.enum(["16:9", "9:16", "1:1"]).default("16:9"),
  duration: z.union([z.literal(3), z.literal(5), z.literal(10)]).default(5),
});
export type HappyhorseFormValues = z.infer<typeof happyhorseFormSchema>;

interface Props {
  projectId: string;
  defaultValues?: Partial<HappyhorseFormValues>;
  onSubmit: (values: HappyhorseFormValues, refs: { assetId: string }[]) => Promise<void> | void;
  submitting: boolean;
}

export function HappyhorseParams({
  projectId,
  defaultValues,
  onSubmit,
  submitting,
}: Props) {
  const form = useForm<HappyhorseFormValues>({
    resolver: zodResolver(happyhorseFormSchema),
    defaultValues: {
      resolution: "720P", ratio: "16:9", duration: 5,
      subject_ids: [], ...defaultValues,
    },
  });

  const subjectIds = form.watch("subject_ids");
  const subjects = useSubjectsByIds(projectId, subjectIds); // batch query
  const referenceAssetIds = useMemo(() => {
    return (subjects.data ?? [])
      .flatMap((s) => s.reference_asset_ids ?? [])
      .filter(Boolean);
  }, [subjects.data]);

  const handle = form.handleSubmit(async (values) => {
    if (referenceAssetIds.length === 0) {
      form.setError("subject_ids", { message: "选中的主体没有任何参考图，无法用于视频生成" });
      return;
    }
    await onSubmit(values, referenceAssetIds.map((id) => ({ assetId: id })));
  });

  return (
    <form onSubmit={handle} className="space-y-4">
      <PromptField {...form.register("prompt")} placeholder="例如：小明翻开书本，抬头微笑" />

      {/* Subject + reference image block */}
      <Field label="主体（参考图来源）" required>
        <Controller
          name="subject_ids"
          control={form.control}
          render={({ field }) => (
            <>
              <SubjectPickerDialog
                projectId={projectId}
                selected={field.value}
                onChange={field.onChange}
              />
              {referenceAssetIds.length > 0 && (
                <ReferenceImagePreview
                  projectId={projectId}
                  assetIds={referenceAssetIds}
                />
              )}
            </>
          )}
        />
      </Field>

      <div className="grid grid-cols-3 gap-3">
        <Field label="分辨率">
          <Controller name="resolution" control={form.control} render={({ field }) => (
            <Select value={field.value} onValueChange={field.onChange}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="720P">720P</SelectItem>
                <SelectItem value="1080P">1080P</SelectItem>
              </SelectContent>
            </Select>
          )} />
        </Field>
        <Field label="画幅">
          <Controller name="ratio" control={form.control} render={({ field }) => (
            <Select value={field.value} onValueChange={field.onChange}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="16:9">16:9</SelectItem>
                <SelectItem value="9:16">9:16</SelectItem>
                <SelectItem value="1:1">1:1</SelectItem>
              </SelectContent>
            </Select>
          )} />
        </Field>
        <Field label="时长（秒）">
          <Controller name="duration" control={form.control} render={({ field }) => (
            <Select value={String(field.value)} onValueChange={(v) => field.onChange(Number(v))}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="3">3</SelectItem>
                <SelectItem value="5">5</SelectItem>
                <SelectItem value="10">10</SelectItem>
              </SelectContent>
            </Select>
          )} />
        </Field>
      </div>

      <Button type="submit" disabled={submitting}>
        {submitting ? "提交中..." : "提交生成"}
      </Button>
    </form>
  );
}
```

### 3.6 SubmitTaskPanel（外壳）

```tsx
// src/components/generation/SubmitTaskPanel.tsx — 约 110 行
export function SubmitTaskPanel({ projectId }: { projectId: string }) {
  const [model, setModel] = useState<ModelChoice | null>(null);
  const submit = useSubmitTask();

  const handleSubmit = async (
    taskType: "image" | "video",
    prompt: string,
    providerParams: Record<string, unknown>,
  ) => {
    if (!model) return;
    try {
      await submit.mutateAsync({
        project_id: projectId,
        account_id: model.accountId,
        model_id: model.modelId,
        task_type: taskType,
        prompt,
        provider_params: providerParams,
      });
      toast.success("任务已提交，可在右侧面板查看进度");
    } catch (err) {
      toast.error(`提交失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  return (
    <Card className="flex h-full flex-col p-4">
      <h2 className="mb-3 text-lg font-semibold">新建生成任务</h2>
      <Field label="模型" required>
        <ModelPicker value={model} onChange={setModel} />
      </Field>

      {model?.modelId === "wan2.7-image-pro" && (
        <Wan27Params
          submitting={submit.isPending}
          onSubmit={(v) =>
            handleSubmit("image", v.prompt, {
              n: v.n, size: v.size,
              enable_sequential: v.enable_sequential,
              negative_prompt: v.negative_prompt ?? null,
            })
          }
        />
      )}

      {model?.modelId === "happyhorse-1.0-r2v" && (
        <HappyhorseParams
          projectId={projectId}
          submitting={submit.isPending}
          onSubmit={(v, refs) =>
            handleSubmit("video", v.prompt, {
              media: refs.map((r) => ({ asset_id: r.assetId, type: "reference_image" })),
              resolution: v.resolution,
              ratio: v.ratio,
              duration: v.duration,
            })
          }
        />
      )}

      {!model && (
        <p className="mt-6 text-sm text-muted-foreground">请先选择模型</p>
      )}
    </Card>
  );
}
```

---

## 4. 任务列表

### 4.1 TaskListPanel

```tsx
// src/components/generation/TaskListPanel.tsx — 约 100 行
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { useTaskList } from "@/hooks/useTasks";
import { TaskCard } from "./TaskCard";
import { EmptyState } from "@/components/common/EmptyState";

type Filter = "all" | "running" | "failed";

export function TaskListPanel({ projectId }: { projectId: string }) {
  const [filter, setFilter] = useState<Filter>("all");
  const tasks = useTaskList({ project_id: projectId });

  const filtered = useMemo(() => {
    if (!tasks.data) return [];
    if (filter === "running") {
      return tasks.data.filter((t) => t.status === "pending" || t.status === "running");
    }
    if (filter === "failed") return tasks.data.filter((t) => t.status === "failed");
    return tasks.data;
  }, [tasks.data, filter]);

  return (
    <Card className="flex h-full flex-col">
      <Tabs value={filter} onValueChange={(v) => setFilter(v as Filter)}>
        <header className="flex items-center justify-between border-b px-4 py-3">
          <h2 className="text-lg font-semibold">任务列表</h2>
          <TabsList>
            <TabsTrigger value="all">全部</TabsTrigger>
            <TabsTrigger value="running">进行中</TabsTrigger>
            <TabsTrigger value="failed">失败</TabsTrigger>
          </TabsList>
        </header>
        <TabsContent value={filter} className="flex-1 overflow-auto p-4">
          {filtered.length === 0 ? (
            <EmptyState
              icon={Wand2}
              title={filter === "all" ? "尚无任务" : "没有匹配的任务"}
              description={filter === "all" ? "在左侧表单提交首个生成任务。" : ""}
            />
          ) : (
            <div className="space-y-3">
              {filtered.map((t) => (
                <TaskCard key={t.id} task={t} projectId={projectId} />
              ))}
            </div>
          )}
        </TabsContent>
      </Tabs>
    </Card>
  );
}
```

### 4.2 TaskCard

```tsx
// src/components/generation/TaskCard.tsx — 约 140 行
import type { GenerationTask } from "@/lib/bindings/commands";
import { TaskStatusBadge } from "./TaskStatusBadge";
import { TaskProgress } from "./TaskProgress";
import { ResultPreviewDialog } from "./ResultPreviewDialog";
import { useCancelTask, useRetryTask } from "@/hooks/useTasks";
import { useResolvedAssetUrl } from "@/hooks/useResolvedAssetUrl";

interface Props {
  task: GenerationTask;
  projectId: string;
}

export function TaskCard({ task, projectId }: Props) {
  const cancel = useCancelTask();
  const retry = useRetryTask();
  const [previewOpen, setPreviewOpen] = useState(false);

  const thumb = useResolvedAssetUrl(
    task.project_root,
    task.result_thumbnail_path,
  );

  const isTerminal = ["success", "failed", "cancelled"].includes(task.status);

  return (
    <div className="rounded-lg border bg-card p-3">
      <div className="flex gap-3">
        {/* Thumbnail (success) or status icon */}
        <div className="h-20 w-20 shrink-0 rounded-md bg-muted overflow-hidden">
          {task.status === "success" && thumb ? (
            <button
              type="button"
              className="h-full w-full"
              onClick={() => setPreviewOpen(true)}
            >
              <img src={thumb} alt="" className="h-full w-full object-cover" />
            </button>
          ) : (
            <div className="flex h-full w-full items-center justify-center text-xs text-muted-foreground">
              {task.task_type === "video" ? "视频" : "图片"}
            </div>
          )}
        </div>

        <div className="flex-1 min-w-0 space-y-1.5">
          <div className="flex items-center gap-2">
            <TaskStatusBadge status={task.status} />
            <span className="text-xs text-muted-foreground">
              {formatRelative(task.created_at)}
            </span>
          </div>
          <p className="line-clamp-2 text-sm">{task.prompt}</p>
          <p className="text-xs text-muted-foreground">
            {task.model_id} · {task.task_type}
          </p>
          {task.status === "running" && (
            <TaskProgress value={task.progress ?? 0} />
          )}
          {task.status === "failed" && task.error_message && (
            <p className="text-xs text-destructive line-clamp-2">{task.error_message}</p>
          )}

          {/* Actions */}
          <div className="flex gap-2 pt-1">
            {!isTerminal && (
              <Button size="sm" variant="outline" onClick={() => cancel.mutate(task.id)}>
                取消
              </Button>
            )}
            {task.status === "failed" && (
              <Button size="sm" variant="outline" onClick={() => retry.mutate(task.id)}>
                重试
              </Button>
            )}
            {task.status === "success" && (
              <Button size="sm" variant="outline" onClick={() => setPreviewOpen(true)}>
                预览
              </Button>
            )}
          </div>
        </div>
      </div>

      {previewOpen && (
        <ResultPreviewDialog
          task={task}
          projectId={projectId}
          onClose={() => setPreviewOpen(false)}
        />
      )}
    </div>
  );
}
```

### 4.3 TaskStatusBadge

```tsx
// src/components/generation/TaskStatusBadge.tsx — 约 50 行
import { cn } from "@/lib/utils";
import type { GenerationTaskStatus } from "@/lib/bindings/commands";

const STYLE: Record<GenerationTaskStatus, { label: string; className: string }> = {
  pending:   { label: "排队中", className: "bg-muted text-muted-foreground" },
  running:   { label: "生成中", className: "bg-blue-100 text-blue-900 dark:bg-blue-950 dark:text-blue-200" },
  success:   { label: "成功",   className: "bg-green-100 text-green-900 dark:bg-green-950 dark:text-green-200" },
  failed:    { label: "失败",   className: "bg-red-100 text-red-900 dark:bg-red-950 dark:text-red-200" },
  cancelled: { label: "已取消", className: "bg-muted text-muted-foreground line-through" },
};

export function TaskStatusBadge({ status }: { status: GenerationTaskStatus }) {
  const s = STYLE[status];
  return (
    <span className={cn("inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium", s.className)}>
      {status === "running" && <Loader2 className="mr-1 h-3 w-3 animate-spin" />}
      {s.label}
    </span>
  );
}
```

### 4.4 useTasks 补丁

`src/hooks/useTasks.ts` 在 spec-15 已起草过外形，spec-18 在该文件下补全。

```ts
// src/hooks/useTasks.ts — 约 130 行
import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  events,
  type GenerationTask,
  type SubmitTaskInput,
  type ListTaskFilter,
} from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

const taskKeys = {
  list: (filter?: ListTaskFilter) =>
    filter ? (["tasks", filter] as const) : (["tasks"] as const),
  detail: (id: string) => ["task", id] as const,
};

export function useTaskList(filter?: ListTaskFilter) {
  return useQuery<GenerationTask[]>({
    queryKey: taskKeys.list(filter),
    queryFn: () => unwrap(commands.listTasks(filter ?? null)),
  });
}

export function useTaskById(id: string | undefined) {
  return useQuery<GenerationTask>({
    queryKey: id ? taskKeys.detail(id) : ["task", "none"],
    queryFn: () => unwrap(commands.getTask(id as string)),
    enabled: !!id,
  });
}

export function useSubmitTask() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: SubmitTaskInput) => unwrap(commands.submitTask(input)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["tasks"] });
    },
  });
}

export function useCancelTask() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.cancelTask(id)),
    onSuccess: (_d, id) => {
      qc.invalidateQueries({ queryKey: ["tasks"] });
      qc.invalidateQueries({ queryKey: taskKeys.detail(id) });
    },
  });
}

export function useRetryTask() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.retryTask(id)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["tasks"] });
    },
  });
}

/**
 * Listen to backend `task-status-changed` events. On every event, invalidate
 * the tasks list cache so all useTaskList consumers refresh.
 *
 * Mounted at GenerationWorkspacePage (single instance) — no per-card listener.
 */
export function useTaskStatusListener(projectId?: string) {
  const qc = useQueryClient();
  useEffect(() => {
    if (!projectId) return;
    const unsub = events.taskStatusChanged.listen((e) => {
      const payload = e.payload;
      qc.invalidateQueries({ queryKey: ["tasks"] });
      qc.invalidateQueries({ queryKey: taskKeys.detail(payload.task_id) });
    });
    return () => {
      void unsub.then((fn) => fn());
    };
  }, [projectId, qc]);
}
```

> spec-15 命令清单需要扩：spec-18 期望 `commands.getTask(id)` 与 `commands.retryTask(id)`，请在 spec-15 的 4 个命令外补这两条。重试的 server-side 实现 = 读出原任务 params_json + prompt → 调 submit_task。

---

## 5. 结果预览

### 5.1 ResultPreviewDialog

```tsx
// src/components/generation/ResultPreviewDialog.tsx — 约 90 行
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { useResolvedAssetUrl } from "@/hooks/useResolvedAssetUrl";
import type { GenerationTask } from "@/lib/bindings/commands";

interface Props {
  task: GenerationTask;
  projectId: string;
  onClose: () => void;
}

export function ResultPreviewDialog({ task, onClose }: Props) {
  // task.result_asset_path is a project-relative path (e.g. "assets/<id>.mp4").
  const url = useResolvedAssetUrl(task.project_root, task.result_asset_path);

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-4xl">
        {!url ? (
          <p className="p-6 text-sm text-muted-foreground">加载中…</p>
        ) : task.task_type === "image" ? (
          <img src={url} alt="" className="w-full max-h-[80vh] object-contain" />
        ) : (
          <video src={url} controls autoPlay className="w-full max-h-[80vh]" />
        )}
        <p className="px-4 pb-3 text-sm text-muted-foreground line-clamp-3">
          {task.prompt}
        </p>
      </DialogContent>
    </Dialog>
  );
}
```

> 文件不存在时（用户手动删了 assets/<id>.mp4 又点预览）：`useResolvedAssetUrl` 返回 URL 但 `<img>` / `<video>` 自身会触发 onError。`ResultPreviewDialog` 在 `<img onError>` / `<video onError>` 上加 fallback：显示「结果文件不在了，可能已被手动删除」并隐藏媒体元素。

### 5.2 ReferenceImagePreview

```tsx
// src/components/generation/ReferenceImagePreview.tsx — 约 50 行
// Shown inside HappyhorseParams, below the subject picker, as a horizontal
// strip of thumbnails. Pure presentational — no upload, no edit.
export function ReferenceImagePreview({ projectId, assetIds }: { projectId: string; assetIds: string[] }) {
  return (
    <div className="flex gap-2 overflow-x-auto py-2">
      {assetIds.map((id) => (
        <ReferenceThumb key={id} projectId={projectId} assetId={id} />
      ))}
    </div>
  );
}

function ReferenceThumb({ projectId, assetId }: { projectId: string; assetId: string }) {
  const asset = useAssetById(assetId);
  const url = useResolvedAssetUrl(asset.data?.project_root, asset.data?.thumbnail_path);
  return (
    <div className="h-16 w-16 shrink-0 rounded-md overflow-hidden bg-muted">
      {url && <img src={url} alt="" className="h-full w-full object-cover" />}
    </div>
  );
}
```

---

## 6. 与 spec-15 后端 IPC 的契约

spec-18 假设 spec-15 的命令面被扩到这套：

| 命令 | 入参 | 出参 | 状态 |
|------|------|------|------|
| `submit_task` | `SubmitTaskInput` | `GenerationTask` | spec-15 已定义 |
| `cancel_task` | `id: String` | `()` | spec-15 已定义 |
| `list_tasks` | `Option<ListTaskFilter>` | `Vec<GenerationTask>` | spec-15 已定义 |
| `get_task` | `id: String` | `GenerationTask` | **spec-18 新增** |
| `retry_task` | `id: String` | `GenerationTask` | **spec-18 新增** |

`SubmitTaskInput` 需要：
```rust
#[derive(Debug, Deserialize, Serialize, TS)]
pub struct SubmitTaskInput {
    pub project_id: String,
    pub account_id: String,
    pub model_id: String,
    pub task_type: GenerationTaskType,   // image | video
    pub prompt: String,
    pub provider_params: serde_json::Value,
}
```

`GenerationTask` 出参需要包含前端用到的字段：`id / project_id / status / task_type / model_id / prompt / progress / error_message / created_at / project_root / result_asset_path / result_thumbnail_path`。spec-15 的查询函数返回的是 DB 行结构，加 `project_root`（从 project 表 join）+ `result_asset_path` / `result_thumbnail_path`（从 asset 表 join）成 view-model。

`retry_task` 行为：DB 读出原 task → 用同样 `account_id / model_id / task_type / prompt / params_json` 调 `submit_task` 流程 → 返回新建 task。原 task 不动（保留为 failed 历史）。

---

## 7. 主体选择器（SubjectPickerDialog）

```tsx
// src/components/generation/SubjectPickerDialog.tsx — 约 110 行
// Reuses SubjectGrid from src/components/subjects with a "select" mode.
// MS1's SubjectGrid only had "view" mode — we add a `selectable` prop in MS1
// component? No — spec-18 instead wraps and overlays a checkbox per card.

import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { useCharacterList } from "@/hooks/useCharacters";  // MS1 hook
import { Button } from "@/components/ui/button";

interface Props {
  projectId: string;
  selected: string[];
  onChange: (ids: string[]) => void;
}

export function SubjectPickerDialog({ projectId, selected, onChange }: Props) {
  const [open, setOpen] = useState(false);
  const characters = useCharacterList(projectId);
  const [draft, setDraft] = useState<string[]>(selected);

  useEffect(() => { if (open) setDraft(selected); }, [open, selected]);

  const toggle = (id: string) =>
    setDraft((prev) =>
      prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id],
    );

  return (
    <>
      <Button type="button" variant="outline" onClick={() => setOpen(true)}>
        {selected.length === 0 ? "选择主体" : `已选 ${selected.length} 个`}
      </Button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="max-w-3xl">
          <DialogTitle>选择主体</DialogTitle>
          <div className="grid grid-cols-3 gap-3 max-h-[60vh] overflow-auto">
            {characters.data?.map((c) => (
              <SelectableSubjectCard
                key={c.id}
                subject={c}
                selected={draft.includes(c.id)}
                onToggle={() => toggle(c.id)}
              />
            ))}
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setOpen(false)}>取消</Button>
            <Button onClick={() => { onChange(draft); setOpen(false); }}>
              确定（{draft.length}）
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
```

> MS2 阶段先只支持 character 一类主体（happyhorse 的实际场景）。scene/prop/costume 在 MS3 之后再扩。

---

## 8. 实时刷新策略

- **单点 listener**：`useTaskStatusListener` 只在 `GenerationWorkspacePage` mount 时挂一个，调用 `invalidateQueries(["tasks"])`。
- **不依赖 event payload 的 progress 增量**：直接 invalidate → 重拉列表。原因：DashScope 异步任务每 5s 才有一次状态变更，进度跳变小，简单 invalidate 比手动合并 cache 鲁棒。
- **进入页面拉一次**：`useTaskList` 自带初始 fetch；离开页面后 TanStack Query 的 GC 在 5min 后清掉缓存。
- **离线重连**：spec-15 的 startup orphan 重置已经把 running 任务恢复成 pending；前端不需特殊处理。

---

## 9. 验收标准

来自任务 6/7/8，对应 UI 表现：

**任务 6（提交表单）**：
- [ ] 模型下拉只显示 `MODEL_CATALOG` 中且对应 provider 已有账号的模型
- [ ] 选 wan2.7-image-pro：表单显示 prompt + 高级参数折叠（n / size / enable_sequential / negative_prompt）
- [ ] 选 happyhorse-1.0-r2v：表单显示 prompt + 主体选择 + resolution/ratio/duration
- [ ] happyhorse 未选主体或选中主体无参考图：提交按钮可点但 Zod 校验报错并阻止提交
- [ ] 提交成功 → toast「任务已提交」+ 表单 reset + 右侧列表立刻多一条 pending 项

**任务 7（任务面板）**：
- [ ] 列表按 `created_at` 倒序
- [ ] 状态徽章 5 种样式区分（pending 灰 / running 蓝+spinner / success 绿 / failed 红 / cancelled 灰删除线）
- [ ] running 任务显示进度条
- [ ] 失败任务显示截断错误信息 + 重试按钮；点击重试后立刻新出一条 pending 项
- [ ] 进行中可取消；取消后 5s 内变为 cancelled 状态
- [ ] tab 过滤「全部 / 进行中 / 失败」工作正常
- [ ] Tauri event 触发后无需手动刷新即可看到状态更新

**任务 8（结果预览）**：
- [ ] 图片任务成功：卡片缩略图可点击 → 弹窗显示原图，最大 80vh
- [ ] 视频任务成功：卡片缩略图可点击 → 弹窗 `<video>` 自动播放（带 controls）
- [ ] 缩略图被手动删除：卡片占位图保留；点击预览 → onError fallback「结果文件不在了」

---

## 10. 测试

`AddAccountDialog.test.tsx` 是已有范式（Vitest + Testing Library + `mock` Tauri commands）。spec-18 应至少覆盖：

- `ModelPicker.test.tsx` — provider 无账号时返回提示，过滤逻辑正确
- `Wan27Params.test.tsx` — 必填校验、提交 payload shape
- `HappyhorseParams.test.tsx` — 主体选了但无参考图时阻断
- `TaskCard.test.tsx` — pending/running/success/failed 各分支 render（mock task fixture）
- `TaskStatusBadge.test.tsx` — 5 种状态样式快照（也可不做，靠组件简单性挡住）

整体 `pnpm test:run` 必须通过；新增组件不允许 `console.error`。

---

## 11. 不在本 spec 范围

- 跨项目任务列表（仅显示当前 `projectId`，spec-15 的 `list_tasks` 已支持过滤）
- 「采用结果」绑定到分镜 → MS3
- 视频帧导出 / 图像编辑 → MS5/MS6
- 队列优先级调整 → MS4 批量功能里

---

## 12. 与现有 MS1 组件的兼容

- `SubjectGrid` / `SubjectCard`（src/components/subjects/）保持原样不改；spec-18 写 `SelectableSubjectCard` 覆盖在外面，避免污染主体库 UI 的 view-only 语义
- `useResolvedAssetUrl` 复用，不改签名
- `Dialog` / `Select` / `Tabs` 等 shadcn 组件均使用现有版本（new-york 风格）

---

## 13. 依赖增量

**spec-18 不引入新 npm 包**。`react-hook-form` / `@hookform/resolvers` / `zod` / `sonner` 已在 MS1 spec-14 引入并 pin 在 package.json。

可能需要新加的 shadcn/ui 组件（用 `pnpm dlx shadcn@latest add <name>` 生成到 `src/components/ui/`，仍不算 npm 依赖）：

- `progress` — `TaskProgress` 进度条
- `badge` — `TaskStatusBadge` 也可以纯手写 `<span>`（spec-18 §4.3 已是手写版），是否补 shadcn `badge` 看实施时的视觉一致性需求

汇总进 `dependencies.md`。
