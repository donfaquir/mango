# SPEC-14: 主体页面与账号管理 UI

## 概述

实现 MS1 的所有前端页面：项目内的"主体库"（含 4 个 Tab：角色 / 场景 / 道具 / 服装），主体详情/编辑页（含参考图预览与拖入），以及设置页下的"账号管理"。所有页面遵循 MS0 spec-08/spec-09 已建立的 layout / 路由 / TanStack Query 风格。

对应 `开发任务.md` 任务 5（主体列表页）+ 任务 6（详情/编辑页）+ 任务 10（账号管理 UI）。

> ⚠️ **破坏性变更**：`ProjectWorkspacePage` 从叶子节点升级为 layout 容器（带 `<Outlet />`）。MS0 中所有指向 `/project/:id` 的链接将自动重定向到 `/project/:id/subjects/character`（含 index route + Navigate）。任何依赖于 `ProjectWorkspacePage` 当前 props/state 的组件需同步重构。SettingsPage 同步改造。

## 设计目标 / 非目标

**目标**：
- 项目工作区内有"主体库"入口，4 类主体 Tab 切换
- 卡片网格展示，含缩略图、名称、简要描述、空状态引导
- 详情页：左侧参考图（拖入 + 预览 + 清空），右侧表单（名称必填）
- 设置页加"账号管理"Tab，按 Provider 分组展示账号
- 添加账号对话框（选 Provider + 输入 label + 输入 API Key）
- 删除角色前，提示其下属服装的级联影响
- 全部页面有加载态（Skeleton）、错误态（Alert）、空态（EmptyState）

**非目标**：
- 不实现搜索（任务 5 提到"按名称过滤"但 MS1 范围内可推迟到 V2，详见 §"与原任务文档的差异"）
- 不实现"账号用量"展示（schema 字段在，但 MS1 没有写入逻辑 → 等 MS2）
- 不实现批量操作（多选、批量删除）
- 不实现"网络连通测试"按钮（MS1 仅有 `verify_storage`，按 spec-13 决策）

## 依赖项变更

**新增 npm 依赖**（需批准）：
```
react-hook-form
@hookform/resolvers
zod
sonner
```

**新增 shadcn/ui 组件**（通过 `pnpm dlx shadcn@latest add ...` 加入）：
- `tabs`、`select`、`alert`、`skeleton`、`form`、`sonner`（或对应 toaster）

> 实施前用 `WebSearch` 核对 `react-hook-form`、`zod`、`sonner` 当时最新 stable 版本。`lucide-react` 已在 deps 中（1.16）；若 spec-14 实施时需要新 icon 且现有版本不含，再决定是否升级。

## 技术方案

### 路由结构

`src/routes.tsx` 增加（**所有 segment 走 path，不用 query param**）：
```tsx
{
  path: "project/:projectId",
  element: <ProjectWorkspacePage />,          // layout 容器 + <Outlet />
  children: [
    { index: true, element: <Navigate to="subjects/character" replace /> },
    {
      path: "subjects",
      children: [
        { index: true, element: <Navigate to="character" replace /> },
        { path: ":kind", element: <SubjectLibraryPage /> },
        { path: ":kind/:subjectId", element: <SubjectDetailPage /> },
      ],
    },
  ],
},
{
  path: "settings",
  element: <SettingsPage />,
  children: [
    { index: true, element: <Navigate to="general" replace /> },
    { path: "general", element: <GeneralSettingsPanel /> },
    { path: "accounts", element: <AccountsPanel /> },
  ],
},
```

> **设计说明**：v1 用 query param `?kind=character` 是错误的。Tab 切换是路由级别的语义切换，路径段更利于浏览器历史、复制粘贴分享、`<Link>` 高亮、route-level 数据加载。

### 目录/文件结构

```
src/
├── pages/
│   ├── SubjectLibraryPage.tsx              # 主体库列表页（容器）
│   ├── SubjectDetailPage.tsx               # 主体详情页（容器）
│   ├── ProjectWorkspacePage.tsx            # 改造：加 outlet + 侧边栏
│   └── SettingsPage.tsx                    # 改造：加 outlet
├── components/
│   ├── subjects/
│   │   ├── SubjectTabs.tsx                 # 4 类 Tab + NavLink 联动
│   │   ├── SubjectGrid.tsx                 # 卡片网格 + 加载/空态
│   │   ├── SubjectCard.tsx                 # 单卡片（用 <Link>）
│   │   ├── SubjectThumbnail.tsx            # 缩略图（含占位）
│   │   ├── CreateSubjectDialog.tsx         # 新建对话框（按 kind 切换字段）
│   │   ├── DeleteSubjectDialog.tsx         # 删除确认（角色含服装计数）
│   │   ├── ReferenceImageDropzone.tsx      # 拖入 + 预览 + 清空
│   │   ├── forms/
│   │   │   ├── CharacterForm.tsx
│   │   │   ├── SceneForm.tsx
│   │   │   ├── PropForm.tsx
│   │   │   └── CostumeForm.tsx             # 含 character_id 关联选择器
│   │   ├── constants.ts                    # SUBJECT_KINDS、路由 helper
│   │   └── types.ts
│   └── accounts/
│       ├── AccountsPanel.tsx               # 设置页面板
│       ├── ProviderGroup.tsx               # 按 Provider 分组
│       ├── AccountRow.tsx                  # 一行账号
│       ├── AddAccountDialog.tsx
│       ├── EditAccountDialog.tsx
│       └── DeleteAccountDialog.tsx
├── hooks/
│   ├── useResolvedAssetUrl.ts              # 把相对路径解析成 tauri:// URL
│   └── useGlobalDropTarget.ts              # 全局拖入事件单订阅 + 派发
└── lib/
    └── subjectKind.ts                      # kind 与 hook/路由的统一映射
```

### 主体库容器（`SubjectLibraryPage.tsx`）

```tsx
export default function SubjectLibraryPage() {
  const { projectId, kind } = useParams<{ projectId: string; kind: SubjectKind }>();
  if (!projectId || !kind || !isValidSubjectKind(kind)) {
    return <Navigate to={`/project/${projectId}/subjects/character`} replace />;
  }

  return (
    <div className="flex flex-col gap-4 p-6">
      <header className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold">主体库</h1>
        <CreateSubjectDialog projectId={projectId} kind={kind} />
      </header>
      <SubjectTabs projectId={projectId} activeKind={kind} />
      <SubjectGrid projectId={projectId} kind={kind} />
    </div>
  );
}
```

> **设计说明**：kind 由路径参数提供；无效值自动回退到 character，UI 无空窗期。

### `SubjectKind` 统一映射（`src/lib/subjectKind.ts`）

```ts
export type SubjectKind = "character" | "scene" | "prop" | "costume";

export const SUBJECT_KIND_LABELS: Record<SubjectKind, string> = {
  character: "角色",
  scene: "场景",
  prop: "道具",
  costume: "服装",
};

export const SUBJECT_KINDS: SubjectKind[] = [
  "character", "scene", "prop", "costume",
];

export function isValidSubjectKind(s: string): s is SubjectKind {
  return (SUBJECT_KINDS as string[]).includes(s);
}

export function subjectListPath(projectId: string, kind: SubjectKind): string {
  return `/project/${projectId}/subjects/${kind}`;
}

export function subjectDetailPath(projectId: string, kind: SubjectKind, id: string): string {
  return `/project/${projectId}/subjects/${kind}/${id}`;
}
```

### SubjectGrid：合规的"按 kind 选 hook"

React Hooks 必须无条件、同序调用。正确做法：**永远同时调用 4 个 hook**，按 kind 控制 `enabled`，最后选一个 query 的结果渲染。各 hook 文件（spec-11）已实现 `enabled: !!projectId` 守卫，传 `undefined` 即不发请求。

```tsx
function SubjectGrid({ projectId, kind }: { projectId: string; kind: SubjectKind }) {
  const characters = useCharacterList(kind === "character" ? projectId : undefined);
  const scenes     = useSceneList(    kind === "scene"     ? projectId : undefined);
  const props      = usePropList(     kind === "prop"      ? projectId : undefined);
  const costumes   = useCostumeList(  kind === "costume"   ? projectId : undefined);

  const query = {
    character: characters,
    scene: scenes,
    prop: props,
    costume: costumes,
  }[kind];

  if (query.isLoading) return <SubjectGridSkeleton />;
  if (query.isError) return <ErrorAlert onRetry={() => query.refetch()} error={query.error} />;
  if (!query.data?.length) return <SubjectEmptyState kind={kind} projectId={projectId} />;

  return (
    <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4">
      {query.data.map((subj) => (
        <SubjectCard key={subj.id} subject={subj} kind={kind} projectId={projectId} />
      ))}
    </div>
  );
}
```

> ⚠️ 不要再写"switch 内调用 hook"的版本（违反 Hooks 规则）。本 spec 仅展示正确实现。

### 卡片与缩略图

```tsx
function SubjectCard({ subject, kind, projectId }: {
  subject: Character | Scene | Prop | Costume;
  kind: SubjectKind;
  projectId: string;
}) {
  return (
    <Link
      to={subjectDetailPath(projectId, kind, subject.id)}
      className="flex flex-col gap-2 p-3 rounded-lg border hover:bg-accent text-left"
    >
      <SubjectThumbnail
        projectId={projectId}
        relativePath={subject.referenceImagePath ?? null}
      />
      <div className="font-medium truncate">{subject.name}</div>
      <div className="text-xs text-muted-foreground line-clamp-2">{subject.description}</div>
    </Link>
  );
}
```

> **设计说明**：用 `<Link>` 而非 `<button>` + `useNavigate`——保留浏览器原生中键打开新窗口、右键复制链接、Cmd/Ctrl+点击等行为。键盘可达性也自动具备。

### `useResolvedAssetUrl` hook

`convertFileSrc` 是同步函数；`@tauri-apps/api/path` 的 `join` 是异步。整体仍是轻量异步操作（无网络），不需要 TanStack Query 的缓存/重试/失效。直接用 `useEffect + useState` 即可：

```ts
// src/hooks/useResolvedAssetUrl.ts
import { useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { join } from "@tauri-apps/api/path";

export function useResolvedAssetUrl(
  projectRoot: string | undefined | null,
  relativePath: string | null,
): string | null {
  const [url, setUrl] = useState<string | null>(null);

  useEffect(() => {
    if (!projectRoot || !relativePath) {
      setUrl(null);
      return;
    }
    let cancelled = false;
    join(projectRoot, relativePath).then((abs) => {
      if (!cancelled) setUrl(convertFileSrc(abs));
    });
    return () => { cancelled = true; };
  }, [projectRoot, relativePath]);

  return url;
}
```

`SubjectThumbnail` 用法：
```tsx
function SubjectThumbnail({ projectId, relativePath }: { projectId: string; relativePath: string | null }) {
  const project = useProject(projectId);
  const url = useResolvedAssetUrl(project.data?.rootPath, relativePath);
  if (!url) return <ImagePlaceholder />;
  return <img src={url} className="w-full aspect-square object-cover rounded" alt="" />;
}
```

> **设计说明**：v1 用 `useQuery` 包装是过度工程（query key 设计也别扭）。本版本 hook 内部仅 `useEffect + useState`，并处理 cancellation 防止旧 promise 覆盖新值。

### `useGlobalDropTarget` hook（解决多 Dropzone 冲突）

`getCurrentWebview().onDragDropEvent` 是全局事件——任何挂载了监听的组件都会收到。当详情页和子组件同时挂载 Dropzone 时，会触发重复 import。

解决：单一全局监听 + dispatch 到当前"活动"目标。

```ts
// src/hooks/useGlobalDropTarget.ts
import { useEffect } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";

type Handler = (paths: string[]) => void;

let activeHandler: Handler | null = null;
let listenerInstalled = false;
let unlistenPromise: Promise<() => void> | null = null;

function installListener() {
  if (listenerInstalled) return;
  listenerInstalled = true;
  unlistenPromise = getCurrentWebview().onDragDropEvent((event) => {
    if (event.payload.type === "drop") {
      activeHandler?.(event.payload.paths);
    }
  });
}

/**
 * 挂载一个"全局唯一"拖入处理器。后挂载的会顶掉前者（先进先出）。
 * 卸载时，如果当前 active handler 仍是自己，则清空。
 */
export function useGlobalDropTarget(handler: Handler, enabled: boolean = true) {
  useEffect(() => {
    if (!enabled) return;
    installListener();
    const prev = activeHandler;
    activeHandler = handler;
    return () => {
      if (activeHandler === handler) {
        activeHandler = prev;
      }
    };
  }, [handler, enabled]);
}
```

> **设计说明**：
> - 全局 active handler 栈式管理：详情页挂载时压栈，子组件不再额外注册。MS1 详情页只挂一个 Dropzone（角色或服装的当前编辑对象），不需要复杂的"区域命中检测"。
> - `handler` 必须 `useCallback` 包裹（见 ReferenceImageDropzone 实现），否则每次 render 都换 handler 引用，破坏栈语义。
> - tauri unlisten 在应用生命周期内不卸载（成本可以忽略）；如果未来需要卸载，可在 module unload 时 await `unlistenPromise`。

### 详情页

`SubjectDetailPage.tsx`：根据 `:kind` 切换渲染对应 form 组件；form 组件内部：
1. `useXxx(subjectId)` 拉数据
2. `react-hook-form` 初始化 defaultValues
3. 顶部"返回主体库"按钮（指向 `subjectListPath(projectId, kind)`）
4. 左侧 `ReferenceImageDropzone`，右侧字段
5. 底部"保存"+"删除"按钮，删除成功后 `navigate(subjectListPath(projectId, kind))`（不用 `navigate(-1)` 避免历史栈污染）
6. 表单提交期间所有按钮 `disabled`（含删除），由 `formState.isSubmitting || deleteMutation.isPending` 控制

CharacterForm 字段：name、description、appearance_prompt。SceneForm 多 environment_prompt。PropForm 仅 name + description。CostumeForm 加 `character_id` 选择器（用 `useCharacterList(projectId)` 拉下拉数据）。

校验（zod）：
```ts
const baseSchema = z.object({
  name: z.string().trim().min(1, "名称必填").max(100),
  description: z.string().max(2000),
});
const characterSchema = baseSchema.extend({
  appearancePrompt: z.string().max(4000),
});
```

提交按钮文案："保存"（编辑）/"创建"（新建模式不在此页，新建走对话框）。

### 参考图拖入组件（`ReferenceImageDropzone.tsx`）

封装 spec-12 §"文件拖入接入" 的逻辑，使用 `useGlobalDropTarget`：

```tsx
type Props = {
  projectId: string;
  projectRoot: string | undefined;
  currentRelativePath: string | null;
  onChange: (relativePath: string | null) => void;
};

export function ReferenceImageDropzone({
  projectId, projectRoot, currentRelativePath, onChange,
}: Props) {
  const importAsset = useImportAsset();

  // 必须 useCallback 包裹，否则栈语义被破坏
  const handleDrop = useCallback(async (paths: string[]) => {
    const path = paths[0];
    if (!path) return;
    try {
      const asset = await importAsset.mutateAsync({ projectId, sourcePath: path });
      onChange(asset.filePath);
    } catch (err) {
      toast.error(`导入失败：${(err as Error).message}`);
    }
  }, [projectId, onChange, importAsset]);

  useGlobalDropTarget(handleDrop, true);

  const url = useResolvedAssetUrl(projectRoot, currentRelativePath);

  return (
    <div className="w-[200px] h-[200px] rounded border-2 border-dashed relative">
      {url ? (
        <img src={url} className="w-full h-full object-cover" alt="参考图" />
      ) : (
        <div className="text-xs text-muted-foreground p-2">拖入图片或点击下方"选择文件"</div>
      )}
      {currentRelativePath && (
        <button
          type="button"
          onClick={() => onChange(null)}
          className="absolute top-1 right-1 ..."
          aria-label="清空参考图"
        >×</button>
      )}
      {/* 下方"选择文件"按钮 → 调用 plugin-dialog 的 open() 单选图片 */}
    </div>
  );
}
```

> **设计说明**：
> - 拖入 → 调 `useImportAsset` → 拿到 Asset → `onChange(asset.filePath)` 由父表单 `react-hook-form` 接管，提交时一并写入。**不在拖放回调里直接调 `update_xxx`**，避免与表单的"未保存修改"语义冲突。
> - 用 `useGlobalDropTarget` 而非自行 `useEffect + onDragDropEvent`——单一监听点 + 栈式 active handler，防止子组件嵌套时重复导入。
> - `handleDrop` 必须 useCallback；其依赖 `onChange`（来自 react-hook-form 的 setter）已稳定，`importAsset` 是 mutation 对象（v5 起 stable identity）。

### 删除确认对话框

`DeleteSubjectDialog.tsx` 接受 `kind` + `subjectId` + `projectId`：
- kind === "character"：先 `useCostumeListByCharacter(projectId, subjectId)` 拉服装数；显示"此角色有 N 件服装将一并删除"；count 加载中按钮禁用
- 其他 kind：简单"确认删除该 X 吗？"
- 确认后调对应 `useDeleteXxx(projectId)` mutation；成功后 `navigate(subjectListPath(projectId, kind))`

### 账号管理面板

`AccountsPanel.tsx`：
```tsx
export function AccountsPanel() {
  const providers = useProviderList();
  const accounts = useAccountList();         // 不传 provider_id，全量
  if (providers.isLoading) return <SettingsSkeleton />;
  if (providers.isError) return <ErrorAlert onRetry={() => providers.refetch()} error={providers.error} />;
  if (!providers.data?.length) {
    // seed 失败的极端场景；提示用户重启或检查日志
    return <Alert variant="destructive">未发现 Provider 配置；请重启应用或查看日志</Alert>;
  }
  return (
    <div className="flex flex-col gap-6">
      {providers.data.map((p) => (
        <ProviderGroup
          key={p.id}
          provider={p}
          accounts={accounts.data?.filter((a) => a.providerId === p.id) ?? []}
        />
      ))}
    </div>
  );
}
```

`ProviderGroup` 内部：
- 标题：`{provider.name}`
- 列表：每个账号一行 `AccountRow`（label / `****last4` / 编辑/删除/校验 三按钮）
- **空 Provider 分组**（该 Provider 下无账号）：标题下方居中提示「尚未配置 {provider.name} 账号」+ 一个"添加账号"按钮（已预选该 provider）
- **非空 Provider 分组**：列表末尾"添加账号"按钮 → 打开 `AddAccountDialog`（已预选 provider）

`AddAccountDialog`：
- `<Select>` 选 Provider（来自 `useProviderList`；预选时禁用以避免误改）
- `<Input>` 填 label（必填）
- `<Input type="password">` 填 API Key（必填）
- 提交期间所有控件 disabled
- 提交 → `useCreateAccount` → 成功 toast + 关闭对话框

`AccountRow` 的"校验"按钮（注意：MS1 仅校验本机存储，不发网络请求）：
- 文案：「校验密钥存储」（不是「测试连接」）
- onClick → `useVerifyApiAccountStorage.mutate(account.id)`
- 成功 toast「密钥存储校验通过」
- 失败 toast 显示后端错误消息
- tooltip / helper text 说明「仅校验本机 keyring；网络连通测试在 MS2 上线」

### 加载/错误/空态规范

| 状态 | UI | 适用页面 |
|---|---|---|
| Loading | shadcn `<Skeleton>` x N（卡片网格 6 个、列表行 3 行） | 所有列表 |
| Error | `<Alert variant="destructive">` + 错误消息 + "重试"按钮（refetch） | 所有页面 |
| Empty | 居中 lucide 图标 + 文字 + 主操作按钮（如"创建第一个角色"） | 列表为空时 |

`<EmptyState>` 通用组件（已在 MS0 spec-09 引入），本 spec 复用。

### TanStack Query invalidation 一览

| 操作 | invalidate 的 key |
|---|---|
| createCharacter | `["characters", projectId]` |
| updateCharacter | `["characters", projectId]` + `["character", id]` |
| deleteCharacter | `["characters", projectId]` + `["costumes", projectId]`（级联） |
| importAsset | `["assets", projectId]` |
| createApiAccount | `["accounts"]` + `["accounts", providerId]` |

由各 hook 文件（spec-11 / spec-12 / spec-13）封装；本 spec 仅消费。

## 测试策略

### 组件单元测试

`src/components/subjects/SubjectCard.test.tsx`：
- mock `useResolvedAssetUrl` 返回固定 URL
- mock `useProject` 返回固定 rootPath
- 渲染含 referenceImage 的 character → 找到 `<img>` 且 src 正确
- 渲染无 referenceImage → 找到占位元素
- 验证 `<a href>` 指向 `subjectDetailPath(projectId, kind, id)`

`src/components/subjects/ReferenceImageDropzone.test.tsx`：
- mock `getCurrentWebview().onDragDropEvent` 触发 drop event
- 验证 `useImportAsset.mutateAsync` 被调用
- 验证 onChange 收到正确路径

`src/components/accounts/AddAccountDialog.test.tsx`：
- 提交空 label / 空 key → 校验错误显示
- 正常提交 → mutation 触发；onSuccess 后对话框关闭
- 提交中所有控件 disabled

`src/hooks/useGlobalDropTarget.test.tsx`：
- 两个组件依次挂载 → 后者获得事件
- 卸载后者 → 前者重新获得事件

### E2E 手测清单

- [ ] 进项目 → 主体库 → 切到"角色"Tab → 列表为空 → 点"创建角色"→ 填名称→ 见到卡片
- [ ] 点开角色 → 拖入一张 PNG → 缩略图出现在 Dropzone 内 → 点"保存"→ 返回列表 → 卡片显示缩略图
- [ ] 点开角色 → 点"删除"→ 见到"无服装将受影响"或"N 件服装将一并删除"
- [ ] 切到 Costume Tab → 创建服装时强制选 character；删除被引用的 Character 后 Costume Tab 自动刷新
- [ ] 路径 `/project/X/subjects/character/Y` 直接粘贴可访问对应角色详情；复制中键打开新 tab 正常
- [ ] 设置页 → 账号管理 → 添加可灵账号 → 列表显示 `****后4位` → 点"校验密钥存储"→ toast 成功
- [ ] 重启应用 → 账号还在；右键 macOS 钥匙串可见条目

## 验收标准

- [ ] 路由 `/project/:id/subjects/:kind` 等 4 个 Tab 可切换；`/project/:id` 自动跳到 character；非法 kind 自动回退
- [ ] 列表加载态、空态、错误态各自走 Skeleton / EmptyState / Alert
- [ ] 详情页拖入 PNG 后立刻预览（不刷新），点保存后刷新列表
- [ ] 删除角色对话框显示其下属服装数量；确认后 Costume Tab 同步更新；删除后导航到列表页（非历史栈回退）
- [ ] 多个 Dropzone 同时挂载时，仅最后挂载的响应拖放（`useGlobalDropTarget` 栈语义验证）
- [ ] 设置页 `/settings/accounts` 按 Provider 分组展示账号；空 Provider 分组有引导
- [ ] 添加账号对话框 API Key 输入框为 password 类型；提交期间控件 disabled；提交后明文不再保留在内存
- [ ] 校验按钮文案为「校验密钥存储」；不出现「测试连接」字样
- [ ] `pnpm typecheck` 与 `pnpm test:run` 全绿
- [ ] 新增 npm 依赖（react-hook-form / zod / @hookform/resolvers / sonner）已批准并写入 package.json

## 依赖关系

- **前置**：spec-10（项目根 `rootPath`）、spec-11（主体 hooks）、spec-12（`useImportAsset` + `resolveAssetUrl`）、spec-13（账号 hooks + `verify_api_account_storage`）
- **后续**：MS2 任务面板 UI 沿用本 spec 的 `<EmptyState>` / `<SubjectCard>` 网格风格

## 与原任务文档的差异

`开发任务.md` 任务 5 验收提到"搜索可按名称过滤"。本 spec **暂不实现**搜索栏，理由：
- MS1 单项目主体规模通常 < 50 个，列表内 Cmd+F 浏览器内置查找够用
- 实现搜索需要决定本地 filter 还是后端 LIKE 查询，进一步引入 debounce 与索引讨论
- 推迟到 V2 与"主体库全局搜索"统一设计

任务 10"账号管理 UI"原文未明确"按 Provider 分组"是必选；本 spec 把它提为强制项，理由：MS2 之后用户会有同 Provider 多账号轮询场景，分组结构对未来的"账号配额条"扩展友好。

任务 10 的"连接测试按钮"按 spec-13 决策调整为「校验密钥存储」（仅校验 keyring 本机存在），MS2 再加真正的网络连通测试。
