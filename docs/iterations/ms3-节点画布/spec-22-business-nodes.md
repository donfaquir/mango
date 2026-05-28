# SPEC-22: 五种业务节点 + 自定义 Edge

## 概述

在 spec-21 的画布骨架之上，实现漫剧业务语义的 5 种自定义节点（Script / Storyboard / Character / Task / Asset）和 3 种自定义 Edge 样式（角色→分镜、分镜→任务、任务→素材），并把节点点击 / 双击 / 右键的交互接入既有数据层。

本 spec **不涉及拖放与外部数据导入**——节点的创建路径限定为「在画布上右键空白处 → 上下文菜单 → 选择节点类型」。从素材面板拖入由 spec-23 实现。

> ⚠️ **破坏性变更**：spec-21 占位的 `nodeTypes` / `edgeTypes` registry 被填充；新增 `NodeKind` / `EdgeKind` 类型定义，**前端类型一旦定型，spec-23 / spec-24 不可破坏性修改**。

## 设计目标 / 非目标

**目标**：

- 5 种节点组件（`<ScriptNode>` / `<StoryboardNode>` / `<CharacterNode>` / `<TaskNode>` / `<AssetNode>`），分别配置 `nodeTypes` registry
- 每种节点的 `data` schema 用 TypeScript discriminated union 表达，序列化时保证可往返
- 节点状态实时数据：
  - `<StoryboardNode>` 显示的 `shot.status` 通过 useShot hook 拉取（spec-20 已实现 `get_shot`）
  - `<CharacterNode>` 显示的 `character.reference_image_path` 通过 useCharacter hook 拉取
  - `<TaskNode>` 监听 MS2 已有的 `task-status-changed` event，实时刷新状态色
  - `<AssetNode>` 调用 `useResolvedAssetUrl`（MS1 已有）展示缩略图
- 3 种 Edge 样式：实线（带角色图标）/ 虚线 / 点线；右键删除
- 节点右键菜单：删除节点 / 查看详情（跳转到对应实体页）
- 「在画布上右键空白处 → 创建节点」上下文菜单
- 节点上双击触发编辑（ScriptNode 展开 textarea；其他节点跳实体页）

**非目标**（明确划归后续 spec）：

- 内部面板 / 系统文件拖入创建节点 → spec-23
- 拖入分镜节点绑定参考图 / 采用视频 → spec-23
- Undo/Redo → spec-24
- 节点级别的"批量选中 + 批量删除" → V2

## 技术方案

### 目录/文件结构

```text
src/components/canvas/
├── nodes/                              # 新建子目录
│   ├── index.ts                        # export nodeTypes（覆盖 spec-21 占位）
│   ├── types.ts                        # NodeKind / NodeData / Node 别名联合
│   ├── ScriptNode.tsx
│   ├── StoryboardNode.tsx
│   ├── CharacterNode.tsx
│   ├── TaskNode.tsx
│   ├── AssetNode.tsx
│   ├── NodeContextMenu.tsx             # 节点右键菜单（删除 / 详情）
│   └── shared/
│       ├── NodeShell.tsx               # 通用外壳：标题栏 + body + 状态色边框
│       ├── StatusDot.tsx
│       └── useCurrentProjectRoot.ts    # useParams + useProject 查项目根路径
├── edges/                              # 新建子目录
│   ├── index.ts                        # export edgeTypes（覆盖 spec-21 占位）
│   ├── types.ts                        # EdgeKind 类型联合（含 asset_to_shot 占位）
│   ├── CharacterToShotEdge.tsx         # 实线 + 角色图标
│   ├── ShotToTaskEdge.tsx              # 虚线
│   ├── TaskToAssetEdge.tsx             # 点线
│   └── EdgeContextMenu.tsx
├── pickers/                            # 新建子目录（spec-22 自带，给 PaneContextMenu 用）
│   ├── ShotPickerDialog.tsx
│   ├── CharacterPickerDialog.tsx
│   ├── TaskPickerDialog.tsx
│   └── AssetPickerDialog.tsx
├── CanvasInner.tsx                     # spec-21 已建，本 spec 加 onPaneContextMenu
├── PaneContextMenu.tsx                 # 新增：画布空白右键菜单（shadcn DropdownMenu 受控弹出）
└── nodeFactory.ts                      # 新增：根据 NodeKind + 实体 id 构造 React Flow Node 对象
```

`src/stores/canvasStore.ts` 也需要增强（新增 `addNode` / `updateNodeData`，并扩展 `onConnect` / `onNodesChange` / `onEdgesChange`），详见后文「canvasStore 增强」「连线行为」「节点 / 连线删除」三节。

### 依赖项变更

无新增 npm / Rust 依赖。

**shadcn/ui 新增**：`context-menu`（节点右键 + 边右键 + 画布空白右键统一用同一个组件）。

```bash
pnpm dlx shadcn@latest add context-menu
```

### 节点 data schema

```ts
// src/components/canvas/nodes/types.ts
import type { Node } from "@xyflow/react";

export type NodeKind = "script" | "storyboard" | "character" | "task" | "asset";

export interface ScriptNodeData {
  kind: "script";
  episodeId: string;
  /// 折叠状态展示前 3 行，展开后全文
  collapsed: boolean;
}

export interface StoryboardNodeData {
  kind: "storyboard";
  shotId: string;
}

export interface CharacterNodeData {
  kind: "character";
  characterId: string;
}

export interface TaskNodeData {
  kind: "task";
  taskId: string;
}

export interface AssetNodeData {
  kind: "asset";
  assetId: string;
}

export type CanvasNodeData =
  | ScriptNodeData
  | StoryboardNodeData
  | CharacterNodeData
  | TaskNodeData
  | AssetNodeData;

// React Flow 12 的 NodeProps 泛型接的是整个 Node 类型（不是 data）。
// 5 个 Node 别名供组件、factory、store 共用。
export type ScriptNode = Node<ScriptNodeData, "script">;
export type StoryboardNode = Node<StoryboardNodeData, "storyboard">;
export type CharacterNode = Node<CharacterNodeData, "character">;
export type TaskNode = Node<TaskNodeData, "task">;
export type AssetNode = Node<AssetNodeData, "asset">;
export type CanvasNode =
  | ScriptNode
  | StoryboardNode
  | CharacterNode
  | TaskNode
  | AssetNode;
```

> **TypeScript 命名说明**：`ScriptNode` 既是 Node 类型别名，也作为下文同名组件 function 的名字。TS 中类型别名仅占类型 namespace，函数声明仅占 value namespace（不会自动派生同名 type），二者**不冲突**，这是 React Flow 12 官方文档示范的写法。

> **设计说明**：每种节点的 `data` 字段只存**实体 id**，不冗余实体内容。实体内容通过 hook 实时拉取——这样实体被改动（如分镜 status 从 draft → ready）后，画布上立即体现，无需手动同步。代价是节点渲染期间会触发 N 个 useQuery；TanStack Query 的缓存层 + `staleTime` 合理设置可以把流量压到极小。
>
> `ScriptNodeData.collapsed` 是唯一的"画布本地状态"——它不属于业务实体，但属于布局，所以存进 node.data，随 layout JSON 持久化。

### canvasStore 增强

spec-21 的 `canvasStore` 暴露了 `init / onNodesChange / onEdgesChange / onConnect / setViewport / reset`。本 spec 增加 2 个新方法，并扩展 3 个已有方法（扩展见后文「连线行为」「节点 / 连线删除」）。

```ts
// 新增方法
addNode: (node: CanvasNode) => {
  if (get().nodes.some((n) => n.id === node.id)) {
    toast.error("该节点已在画布上");
    return;
  }
  set({ nodes: [...get().nodes, node] });
},

updateNodeData: <T extends CanvasNodeData>(
  id: string,
  partial: Partial<T>,
) =>
  set({
    nodes: get().nodes.map((n) =>
      n.id === id ? { ...n, data: { ...n.data, ...partial } } : n,
    ),
  }),
```

> **设计说明**：React Flow 12 对重复 id 的 nodes 不会抛错，只会渲染异常 + 控制台 warning。`addNode` 在内部做 id dedup 是必要的——同一 shot/character/asset 被加两次时，第二次直接弹 toast 拒绝。`updateNodeData` 走泛型，给 ScriptNode 切换 `collapsed`、未来其他节点改局部状态都能复用，且变更触发 spec-21 的 useAutoSave 写回。

### 5 种节点组件

#### NodeShell 通用外壳

```tsx
// src/components/canvas/nodes/shared/NodeShell.tsx
type StatusColor = "gray" | "blue" | "yellow" | "green" | "red";

// 显式 mapping：Tailwind v4 扫描器只匹配字面量字符串，
// 模板拼接的 `border-${color}-300` 不会被收进产物 CSS。
const STATUS_BORDER: Record<StatusColor, string> = {
  gray: "border-gray-300",
  blue: "border-blue-300",
  yellow: "border-yellow-300",
  green: "border-green-300",
  red: "border-red-300",
};

interface NodeShellProps {
  title: string;
  statusColor?: StatusColor;
  selected?: boolean;
  children: React.ReactNode;
  onContextMenu?: (e: React.MouseEvent) => void;
}

export function NodeShell({ title, statusColor = "gray", selected, children, onContextMenu }: NodeShellProps) {
  return (
    <div
      onContextMenu={onContextMenu}
      className={cn(
        "min-w-[180px] rounded-lg border-2 bg-card shadow-sm transition-all",
        selected ? "border-primary" : STATUS_BORDER[statusColor],
      )}
    >
      <header className="border-b px-3 py-1.5 text-xs font-medium text-muted-foreground">{title}</header>
      <div className="p-3">{children}</div>
      <Handle type="target" position={Position.Top} />
      <Handle type="source" position={Position.Bottom} />
    </div>
  );
}
```

#### useCurrentProjectRoot

`useResolvedAssetUrl(projectRoot, relativePath)` 需要 project 的 `root_path`。CharacterNode / AssetNode 都要用，所以收口到一个 hook：

```ts
// src/components/canvas/nodes/shared/useCurrentProjectRoot.ts
export function useCurrentProjectRoot(): string | null {
  const { projectId } = useParams<{ projectId: string }>();
  const project = useProject(projectId);
  return project.data?.root_path ?? null;
}
```

#### ScriptNode

```tsx
import type { ScriptNode as ScriptNodeT } from "./types";

export function ScriptNode({ id, data, selected }: NodeProps<ScriptNodeT>) {
  const episode = useEpisode(data.episodeId);
  const text = episode.data?.script_text ?? "";
  const preview = text.split("\n").slice(0, data.collapsed ? 3 : Infinity).join("\n");
  const updateNodeData = useCanvasStore((s) => s.updateNodeData);

  return (
    <NodeShell title={`剧本：${episode.data?.title ?? "..."}`} selected={selected}>
      <pre
        className="whitespace-pre-wrap text-sm"
        onDoubleClick={() =>
          updateNodeData<ScriptNodeData>(id, { collapsed: !data.collapsed })
        }
      >
        {preview || <span className="text-muted-foreground">（暂无剧本内容）</span>}
      </pre>
    </NodeShell>
  );
}
```

> **设计说明**：ScriptNode 的"展开/折叠"切换是 `updateNodeData` 而非 React 局部 state —— 折叠状态写入 node.data，跟随 layout 持久化。next session 打开画布看到的是上次的展开状态。

#### StoryboardNode

```tsx
import type { StoryboardNode as StoryboardNodeT } from "./types";

export function StoryboardNode({ data, selected }: NodeProps<StoryboardNodeT>) {
  const shot = useShot(data.shotId);
  const statusColor: Record<ShotStatus, "gray" | "blue" | "yellow" | "green"> = {
    draft: "gray", ready: "blue", generating: "yellow", done: "green",
  };
  if (!shot.data) {
    return (
      <NodeShell title="分镜（已删除）" statusColor="red" selected={selected}>
        <p className="text-xs text-muted-foreground">原分镜已被删除，请从画布上移除该节点。</p>
      </NodeShell>
    );
  }
  return (
    <NodeShell
      title={`分镜 #${shot.data.order_index}`}
      statusColor={statusColor[shot.data.status]}
      selected={selected}
    >
      <p className="text-sm">{shot.data.summary || <em className="text-muted-foreground">无摘要</em>}</p>
      {shot.data.dialogue ? <p className="mt-2 text-xs text-muted-foreground">「{shot.data.dialogue}」</p> : null}
      {shot.data.duration_sec ? <p className="mt-1 text-xs">{shot.data.duration_sec.toFixed(1)}s</p> : null}
    </NodeShell>
  );
}
```

#### CharacterNode

```tsx
import type { CharacterNode as CharacterNodeT } from "./types";

export function CharacterNode({ data, selected }: NodeProps<CharacterNodeT>) {
  const character = useCharacter(data.characterId);
  const projectRoot = useCurrentProjectRoot();
  const refUrl = useResolvedAssetUrl(projectRoot, character.data?.reference_image_path ?? null);
  return (
    <NodeShell title="角色" selected={selected}>
      <div className="flex items-center gap-3">
        {refUrl ? (
          <img src={refUrl} className="h-12 w-12 rounded object-cover" alt={character.data?.name} />
        ) : (
          <div className="h-12 w-12 rounded bg-muted text-center text-xs leading-[3rem]">无图</div>
        )}
        <div>
          <p className="font-medium">{character.data?.name ?? "..."}</p>
          <p className="line-clamp-1 text-xs text-muted-foreground">{character.data?.appearance_prompt}</p>
        </div>
      </div>
    </NodeShell>
  );
}
```

#### TaskNode

```tsx
import type { TaskNode as TaskNodeT } from "./types";

export function TaskNode({ data, selected }: NodeProps<TaskNodeT>) {
  const task = useTaskById(data.taskId);
  const statusColor = {
    pending: "gray", running: "yellow", success: "green", failed: "red", cancelled: "gray",
  } as const;
  return (
    <NodeShell
      title={`任务：${task.data?.model_id ?? "..."}`}
      statusColor={task.data ? statusColor[task.data.status] : "gray"}
      selected={selected}
    >
      <TaskStatusBadge status={task.data?.status} />
      {task.data?.status === "failed" ? (
        <Button size="sm" variant="ghost" onClick={() => openDiagnostics(task.data.id)}>诊断</Button>
      ) : null}
    </NodeShell>
  );
}
```

> **设计说明**：`useTaskById`（`src/hooks/useTasks.ts`）是 MS2 已实现的 hook，本 spec 直接复用——MS2 的 `useTaskStatusListener` 在 `task-status-changed` event 触发时 invalidateQueries(taskKeys.all)，所有 TaskNode 会自动重渲染。为限制无差别 invalidate 带来的 N 个节点同时 re-fetch，TaskNode 内层用 `select` 取最小子集（status / model_id / id）。

#### AssetNode

```tsx
import type { AssetNode as AssetNodeT } from "./types";

export function AssetNode({ data, selected }: NodeProps<AssetNodeT>) {
  const asset = useAsset(data.assetId);
  const projectRoot = useCurrentProjectRoot();
  const thumbUrl = useResolvedAssetUrl(
    projectRoot,
    asset.data?.thumbnail_path ?? asset.data?.file_path ?? null,
  );
  return (
    <NodeShell title="素材" selected={selected}>
      {thumbUrl ? (
        <img src={thumbUrl} className="h-24 w-full rounded object-cover" alt={asset.data?.original_name} />
      ) : (
        <div className="h-24 rounded bg-muted" />
      )}
      <p className="mt-2 truncate text-xs">{asset.data?.original_name}</p>
      {asset.data?.asset_type === "video" ? (
        <Button size="sm" variant="ghost" onClick={() => openPreview(asset.data!)}>播放</Button>
      ) : null}
    </NodeShell>
  );
}
```

### Edge 样式

```ts
// src/components/canvas/edges/types.ts
export type EdgeKind =
  | "character_to_shot"
  | "shot_to_task"
  | "task_to_asset"
  | "asset_to_shot"; // 预留：spec-23 拖放素材到分镜节点上时使用，本 spec 不实现 UI / inferEdgeKind 分支
```

> **类型占位说明**：spec-22 明确声明「前端类型一旦定型，后续 spec 不可破坏性修改」（见概述）。spec-23 已计划在拖放阶段引入 asset→shot 关联表达，所以本 spec 把 `asset_to_shot` 字面量先放进 EdgeKind 联合，避免 spec-23 落地时回头改 spec-22 的 schema。实际的 EdgeComponent / inferEdgeKind 分支由 spec-23 实现。

```tsx
// CharacterToShotEdge.tsx：实线 + 角色图标
export function CharacterToShotEdge({ id, sourceX, sourceY, targetX, targetY, style, data }: EdgeProps) {
  const [edgePath, labelX, labelY] = getBezierPath({ sourceX, sourceY, targetX, targetY });
  return (
    <>
      <path id={id} d={edgePath} className="react-flow__edge-path stroke-2" style={style} />
      <foreignObject x={labelX - 12} y={labelY - 12} width={24} height={24}>
        <Users className="h-6 w-6 text-blue-500" />
      </foreignObject>
    </>
  );
}

// ShotToTaskEdge：虚线
// TaskToAssetEdge：点线
```

> **设计说明**：Edge 上不显示文本 label——业务关系类型已通过 edge 的 `type` 字段隐式传达，再加文字就冗余了。如果未来用户反馈无法区分线型，再补 label。

### Picker 对话框

PaneContextMenu「添加 X 节点」要求用户从**已有实体**里选一个（节点 ↔ 实体严格 1:1）。spec-20 的 `ShotListPanel` 不是选择器、改造侵入式较大，本 spec 新建 4 个独立 Picker：

| Picker | 列表数据 hook | 空态行为 |
|---|---|---|
| `ShotPickerDialog` | `useShotList(episodeId)` | 「该剧集还没有分镜」+ 跳 `EpisodeDetailPage` 按钮 |
| `CharacterPickerDialog` | `useCharacterList(projectId)` | 「该项目还没有角色」+ 跳 `SubjectLibraryPage` 按钮 |
| `TaskPickerDialog` | `useTaskList({ project_id })` | 「该项目还没有任务」+ 跳 `GenerationWorkspacePage` 按钮 |
| `AssetPickerDialog` | `useAssetList({ project_id })` | 「该项目还没有素材」+ 跳 `AssetLibraryPage` 按钮 |

统一接口：

```ts
interface PickerProps {
  scopeId: string;            // episodeId（ShotPicker）或 projectId（其余三个）
  open: boolean;
  onPick: (id: string) => void;
  onClose: () => void;
}
```

实现要点：基于 shadcn `Dialog` + 顶部搜索框 + 滚动列表 + 「选择」按钮；列表行用现有实体卡片样式（缩略图 + 标题 + 副标题）。每个 Picker 文件 < 120 行。

> **设计说明**：把 Picker 放进 spec-22 而非推迟到 spec-23，是因为不做 Picker 的话 PaneContextMenu 的 4/5 个菜单项无法工作，spec-22 的「5 种节点全部可创建」目标无法验收。spec-23 拖放上线后，拖入路径复用同一 `nodeFactory`，Picker 路径继续保留作为「不想离开画布去找东西时的兜底」。

### 节点工厂 + 上下文菜单

```ts
// src/components/canvas/nodeFactory.ts

// 所有 factory 都用「业务 id 作为节点 id」统一策略：
// 同一实体（剧集 / 分镜 / 角色 / 任务 / 素材）在画布上只能有一个节点。
// addNode 的 dedup 会拒绝第二次添加并 toast 提示。

export function createScriptNode(episodeId: string, position: XYPosition): ScriptNode {
  return {
    id: `script-${episodeId}`,
    type: "script",
    position,
    data: { kind: "script", episodeId, collapsed: true },
  };
}

export function createStoryboardNode(shotId: string, position: XYPosition): StoryboardNode {
  return {
    id: `storyboard-${shotId}`,
    type: "storyboard",
    position,
    data: { kind: "storyboard", shotId },
  };
}

// createCharacterNode(characterId) → id: `character-${characterId}`
// createTaskNode(taskId)            → id: `task-${taskId}`
// createAssetNode(assetId)          → id: `asset-${assetId}`
```

```tsx
// CanvasInner.tsx 新增 state + handler，把空白右键的 client/pane 坐标交给 PaneContextMenu
const [paneMenu, setPaneMenu] = useState<
  { client: { x: number; y: number }; pane: { x: number; y: number } } | null
>(null);

<ReactFlow
  ...
  onPaneContextMenu={(event) => {
    event.preventDefault();
    const pane = reactFlowInstance.screenToFlowPosition({ x: event.clientX, y: event.clientY });
    setPaneMenu({ client: { x: event.clientX, y: event.clientY }, pane });
  }}
/>

{paneMenu && (
  <PaneContextMenu
    clientX={paneMenu.client.x}
    clientY={paneMenu.client.y}
    paneX={paneMenu.pane.x}
    paneY={paneMenu.pane.y}
    onClose={() => setPaneMenu(null)}
  />
)}
```

```tsx
// PaneContextMenu.tsx —— shadcn DropdownMenu 受控弹出
// 画布空白右键场景下，shadcn 的 ContextMenu 需要包裹整片区域才能触发；
// 这里只想在「点击的那个坐标」上弹一次菜单，因此用 DropdownMenu + 不可见 anchor。
type Props = {
  clientX: number;
  clientY: number;
  paneX: number;
  paneY: number;
  onClose: () => void;
};

export function PaneContextMenu({ clientX, clientY, paneX, paneY, onClose }: Props) {
  const { projectId, episodeId } = useParams<{ projectId: string; episodeId: string }>();
  const addNode = useCanvasStore((s) => s.addNode);
  const [picker, setPicker] = useState<NodeKind | null>(null);
  const pos = { x: paneX, y: paneY };

  const closeAll = () => { setPicker(null); onClose(); };

  return (
    <>
      <DropdownMenu open onOpenChange={(o) => !o && onClose()}>
        <DropdownMenuTrigger asChild>
          <span
            className="pointer-events-none fixed h-0 w-0"
            style={{ left: clientX, top: clientY }}
            aria-hidden
          />
        </DropdownMenuTrigger>
        <DropdownMenuContent side="bottom" align="start" sideOffset={4}>
          <DropdownMenuItem onClick={() => { addNode(createScriptNode(episodeId, pos)); closeAll(); }}>
            新建剧本节点
          </DropdownMenuItem>
          <DropdownMenuItem onClick={() => setPicker("storyboard")}>添加分镜节点</DropdownMenuItem>
          <DropdownMenuItem onClick={() => setPicker("character")}>添加角色节点</DropdownMenuItem>
          <DropdownMenuItem onClick={() => setPicker("task")}>添加任务节点</DropdownMenuItem>
          <DropdownMenuItem onClick={() => setPicker("asset")}>添加素材节点</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      {picker === "storyboard" && (
        <ShotPickerDialog
          scopeId={episodeId}
          open
          onPick={(shotId) => { addNode(createStoryboardNode(shotId, pos)); closeAll(); }}
          onClose={closeAll}
        />
      )}
      {picker === "character" && (
        <CharacterPickerDialog
          scopeId={projectId}
          open
          onPick={(id) => { addNode(createCharacterNode(id, pos)); closeAll(); }}
          onClose={closeAll}
        />
      )}
      {/* TaskPickerDialog / AssetPickerDialog 同款 */}
    </>
  );
}
```

> **设计说明**：节点 / 边右键菜单（NodeContextMenu / EdgeContextMenu）走 shadcn 的 `ContextMenu`（声明式包裹），因为它们触发区域固定为节点 / 边本身。画布空白右键的坐标是动态的，所以 PaneContextMenu 用 `DropdownMenu` + 受控 `open` + 不可见 anchor 定位——避免给整个画布加 ContextMenu 包裹器（会和 RF 自身的事件冲突）。

### 连线行为

React Flow 12 的 `onConnect` 在 spec-21 的 store 已实现 `addEdge`。本 spec 在 `onConnect` 上叠加业务规则：

```ts
// canvasStore.ts 增强 onConnect —— 保持同步签名（OnConnect 类型要求），异步副作用走 .catch
onConnect: (params: Connection) => {
  const sourceNode = get().nodes.find((n) => n.id === params.source);
  const targetNode = get().nodes.find((n) => n.id === params.target);
  if (!sourceNode || !targetNode) return;

  const edgeKind = inferEdgeKind(sourceNode.data, targetNode.data);
  if (!edgeKind) {
    toast.error(`不支持 ${sourceNode.data.kind} → ${targetNode.data.kind} 的连线`);
    return;
  }

  // 自己生成 edge id —— Connection 没有 id 字段，addEdge 内部生成的 id 拿不到，
  // 所以必须显式传 id 才能在回滚时定位到这条 edge。
  const edgeId = `edge-${crypto.randomUUID()}`;
  set({
    edges: addEdge({ ...params, id: edgeId, type: edgeKind }, get().edges),
  });

  if (edgeKind === "character_to_shot") {
    const { characterId } = sourceNode.data as CharacterNodeData;
    const { shotId } = targetNode.data as StoryboardNodeData;
    commands
      .linkShotSubject(shotId, characterId, "character")
      .catch((e) => {
        toast.error(`绑定角色到分镜失败：${e}`);
        set({ edges: get().edges.filter((edge) => edge.id !== edgeId) });
      });
  }
},

function inferEdgeKind(s: CanvasNodeData, t: CanvasNodeData): EdgeKind | null {
  if (s.kind === "character" && t.kind === "storyboard") return "character_to_shot";
  if (s.kind === "storyboard" && t.kind === "task") return "shot_to_task";
  if (s.kind === "task" && t.kind === "asset") return "task_to_asset";
  return null;
}
```

> **设计说明**：业务连线（character → storyboard）**同时**写画布 edge JSON **和** shot_character 关联表。两者的一致性：
> - 画布 edge 是 UI 表达，关联表是数据真相
> - 用户用 edge 删除（按 Delete 键）时，反向 unlink_shot_subject
> - 关联表被外部修改（如未来增加"在分镜详情页绑定角色"功能）后，下次打开画布会显示 edge 缺失——不自动同步，用户可手动补连
>
> 这个权衡选「画布与关联表同步双写」而非「画布只看 UI、关联表只看数据」，理由：(1) 画布是 MS3 用户的主交互入口，连线即意图；(2) MS2 已有的「为分镜提交任务」需要 shot_character 数据决定 character 的 reference_image 走向 happyhorse；(3) 不双写就要在 EpisodeDetailPage 加另一套绑定 UI，工作量翻倍。

### 节点 / 连线删除

业务关联（如 `shot_character`）和画布 edge 双写后，删除必须双路径反向 unlink：

1. **删 character 节点 / storyboard 节点** → 拦截 `onNodesChange.remove`，找到任一端为该节点的 `character_to_shot` edge，反向调 `unlink_shot_subject`。
2. **删 edge**（用户按 Delete 键 或 EdgeContextMenu「删除连线」）→ 拦截 `onEdgesChange.remove`，对 `character_to_shot` 类型直接反向 unlink。

```ts
// canvasStore.ts 增强 onNodesChange
onNodesChange: (changes: NodeChange[]) => {
  for (const change of changes) {
    if (change.type !== "remove") continue;
    const node = get().nodes.find((n) => n.id === change.id);
    if (!node) continue;
    // 任一端是被删节点的 character_to_shot edges 都要反向 unlink
    const affected = get().edges.filter(
      (e) =>
        e.type === "character_to_shot" &&
        (e.source === node.id || e.target === node.id),
    );
    for (const e of affected) {
      const char = get().nodes.find((n) => n.id === e.source);
      const shot = get().nodes.find((n) => n.id === e.target);
      if (char?.data.kind === "character" && shot?.data.kind === "storyboard") {
        commands
          .unlinkShotSubject(shot.data.shotId, char.data.characterId, "character")
          .catch(console.error);
      }
    }
  }
  set({ nodes: applyNodeChanges(changes, get().nodes) });
},

// canvasStore.ts 增强 onEdgesChange
onEdgesChange: (changes: EdgeChange[]) => {
  for (const change of changes) {
    if (change.type !== "remove") continue;
    const edge = get().edges.find((e) => e.id === change.id);
    if (edge?.type !== "character_to_shot") continue;
    const sourceNode = get().nodes.find((n) => n.id === edge.source);
    const targetNode = get().nodes.find((n) => n.id === edge.target);
    if (
      sourceNode?.data.kind === "character" &&
      targetNode?.data.kind === "storyboard"
    ) {
      commands
        .unlinkShotSubject(
          targetNode.data.shotId,
          sourceNode.data.characterId,
          "character",
        )
        .catch(console.error);
    }
  }
  set({ edges: applyEdgeChanges(changes, get().edges) });
},
```

> **设计说明**：
> - 删 character / storyboard 节点是「整批 unlink」；删 edge 是「单条 unlink」——两条路径解耦避免漏一边。
> - 删 task 节点 **不** 级联停 generation 任务：任务有独立生命周期（MS2 已实现的 `task_engine`），画布只是视图。如果用户想停任务，去 `GenerationWorkspacePage` 操作；spec-24 可考虑加 NodeContextMenu「取消任务」入口。
> - 删 asset 节点 **不** 触发文件删除：素材删除走 `AssetLibraryPage`。

### 错误场景

| 场景 | 行为 |
|---|---|
| 节点 data 引用的实体已被删除（如 shot_id 对应的 shot 被删） | useShot 返回 NotFound；StoryboardNode 显示「[分镜已删除]」 + 一个「从画布移除」按钮 |
| 连线源/目标类型不被支持（如 asset → character） | toast「不支持的连线类型」，不写入 edges |
| `link_shot_subject` IPC 失败 | toast 错误 + 回滚刚加的 edge |
| 节点 data schema 不认识（如未来引入新 NodeKind 后回滚） | nodeTypes registry 命中失败，React Flow 渲染 `default` 节点显示 raw data；不崩溃 |
| 右键菜单点击「添加分镜节点」但项目下没有 shot | ShotPickerDialog 显示空态 + 「新建一个分镜」按钮（跳 EpisodeDetailPage） |
| 任何节点重复创建（用户两次添加同一实体） | factory 用业务 id 作为节点 id，`canvasStore.addNode` 内部做 dedup → toast「该节点已在画布上」并拒绝添加（React Flow 12 对重复 id 不会抛错，必须在 store 层拦截） |
| Picker 数据加载失败 | Picker 内部显示 ErrorAlert + 重试按钮（与项目里其它 list 页统一样式） |
| 项目 root_path 未加载完（useCurrentProjectRoot 返回 null） | CharacterNode / AssetNode 显示「无图」/「无缩略图」占位，不渲染 broken img |

### 测试策略

#### 前端单元测试

`src/components/canvas/nodes/ScriptNode.test.tsx`：
- 渲染折叠态时只显示前 3 行
- 双击触发 `updateNodeData({ collapsed: false })`
- 引用的 episode 不存在时显示空态

`src/components/canvas/nodes/StoryboardNode.test.tsx`：
- 4 种 status 分别渲染对应颜色边框
- dialogue 为空时不渲染"「」"包裹

`src/components/canvas/nodes/CharacterNode.test.tsx`：
- 无参考图时显示「无图」占位符
- useResolvedAssetUrl 返回 url 时 img.src 正确

`src/components/canvas/nodes/TaskNode.test.tsx`：
- failed 状态显示「诊断」按钮
- pending/running 不显示

`src/components/canvas/edges/CharacterToShotEdge.test.tsx`：
- 渲染包含 `<Users>` 图标
- 路径 d 属性非空

`src/components/canvas/nodeFactory.test.ts`：
- createStoryboardNode 的 id 是 `storyboard-${shotId}`
- createScriptNode 的 id 是 `script-${episodeId}`（与 storyboard 同款，业务 id）
- 所有 5 个 factory 返回的 node.data.kind 与 NodeKind 一致

`src/stores/canvasStore.addNode.test.ts`：
- addNode 写入新节点
- addNode 对同 id 拒绝 + toast 提示
- updateNodeData 合并 partial 而不覆盖整个 data

`src/stores/canvasStore.connect.test.ts`（mock `@/lib/bindings/commands`）：
- inferEdgeKind 三种合法组合返回对应 kind
- 非法组合返回 null + toast
- character→storyboard 连线调 linkShotSubject 且使用 self-generated edgeId
- linkShotSubject reject 时该 edge 被精准移除（其他 edge 不受影响）

`src/stores/canvasStore.delete.test.ts`（mock `@/lib/bindings/commands`）：
- 删 character 节点 → unlinkShotSubject 被调用一次
- 删 storyboard 节点 → unlinkShotSubject 被调用一次
- 删 character_to_shot edge（onEdgesChange.remove） → unlinkShotSubject 被调用一次
- 删 shot_to_task edge → 不调任何 unlink

`src/components/canvas/pickers/ShotPickerDialog.test.tsx`：
- 渲染分镜列表 + 搜索过滤
- 空态显示「跳 EpisodeDetailPage」按钮
- onPick 回调被调用

#### 手动验证清单

> DB 查询示例：`sqlite3 "$HOME/Library/Application Support/com.mango.app/mango.db" "SELECT * FROM shot_character;"`（macOS dev 路径）

- [ ] 画布空白右键 → 弹出 5 个「添加节点」菜单项
- [ ] 添加一个 ScriptNode → 看到 episode.script_text 前 3 行
- [ ] 双击 ScriptNode → 展开看到全文 → 刷新页面 → 仍保持展开
- [ ] 同一 episode 再次「新建剧本节点」 → toast「该节点已在画布上」
- [ ] 添加 StoryboardNode（选 shot status=draft）→ 看到灰色边框
- [ ] 在 EpisodeDetailPage 改 shot status 为 ready → 回画布 → StoryboardNode 边框变蓝
- [ ] 添加 CharacterNode + StoryboardNode → 拖连线 → 看到实线 + 角色图标 → DB 查 `shot_character` 新增一行
- [ ] 选中该条 edge → 按 Delete → DB 查 `shot_character` 对应行消失（edge-only 删除路径）
- [ ] 再连一次同样的 edge → 删 CharacterNode → DB 查 `shot_character` 行也消失（节点删除路径）
- [ ] 添加 TaskNode（选 running 状态的任务） → 看到黄色边框 → 任务完成 → 边框变绿
- [ ] 添加 AssetNode（选 video） → 看到「播放」按钮 → 点击弹 ResultPreviewDialog（复用 MS2 已有）
- [ ] 项目无任何分镜时打开 ShotPickerDialog → 看到空态 + 跳 EpisodeDetailPage 按钮

## 验收标准

- [ ] 5 种节点组件全部实现，`NodeProps<XxxNode>` 泛型正确，`data.kind` 字段做 discriminated union
- [ ] 3 种 Edge 样式可视化区分（线型 + 图标），EdgeKind 含 `asset_to_shot` 占位
- [ ] `nodeTypes` / `edgeTypes` registry 正确注册（覆盖 spec-21 的空 registry）
- [ ] `canvasStore` 新增 `addNode` / `updateNodeData`，并扩展 `onConnect` / `onNodesChange` / `onEdgesChange`
- [ ] PaneContextMenu（DropdownMenu）+ NodeContextMenu（ContextMenu）+ EdgeContextMenu（ContextMenu）三个右键菜单工作
- [ ] 4 个 Picker 对话框工作，空态有跳转引导
- [ ] CharacterToShotEdge 连线同步写 `shot_character` 表；失败时精准回滚刚加的那条 edge（不影响其它）
- [ ] 删除连线（Delete 键 / EdgeContextMenu）和删除节点（character / storyboard）都触发反向 `unlink_shot_subject`
- [ ] 引用实体被删时节点显示空态而非崩溃
- [ ] NodeShell 状态色用显式 mapping，无动态拼接的 Tailwind 类名
- [ ] `pnpm test:run` 至少新增 20 个前端测试（5 个节点 × 各 2-3 + edges 1 + factory 1 + store 3 + picker 1）
- [ ] `pnpm typecheck` 干净
- [ ] 手动验证清单全过

## 依赖关系

- **前置**：spec-20（episode / shot / character / link_shot_subject 全部 IPC）+ spec-21（CanvasInner / canvasStore / nodeTypes registry）
- **平行**：spec-23（拖放）—— 拖放产生节点的 factory 复用本 spec 的 `nodeFactory.ts`，因此**建议 spec-22 先合**
- **后续**：
  - SPEC-24 Undo/Redo 必须覆盖节点 data 的变化（`updateNodeData` 触发 zundo snapshot）

## 与原任务文档的差异

合并自原任务 2（ScriptNode）+ 任务 3（StoryboardNode）+ 任务 4（CharacterNode）+ 任务 5（TaskNode）+ 任务 6（AssetNode）+ 任务 7（自定义 Edge）。

- **原任务 4 提到"连线：可连接到 StoryboardNode"但没说怎么写 shot_character 关联表**：本 spec 明确"画布 edge 与 shot_character 双写"的策略与双路径反向 unlink（节点删 + edge 删）
- **原任务 8 提到"拖放到分镜节点上：绑定为该分镜的参考图/采用视频"**：拖放本身由 spec-23 实现；本 spec 在 `EdgeKind` 联合里**预留** `asset_to_shot` 字面量，避免 spec-23 引入新 kind 时破坏 spec-22 的类型 contract。实际 EdgeComponent / inferEdgeKind 分支由 spec-23 实现
- **原任务 5 提到"实时状态更新：监听 Tauri event"**：复用 MS2 已建的 `useTaskStatusListener` + invalidateQueries 机制，本 spec 不重复造轮子
- **Picker 对话框（4 个）纳入 spec-22 而非推迟到 spec-23**：不做 Picker 时 PaneContextMenu 的 4/5 个菜单项无法工作，spec-22 的「5 种节点全部可创建」目标无法验收。spec-23 拖放上线后两条路径并存（拖放 + 右键选择），都复用同一 `nodeFactory`
