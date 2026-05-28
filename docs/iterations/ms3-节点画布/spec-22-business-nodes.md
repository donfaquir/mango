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
│   ├── types.ts                        # NodeKind / NodeData 类型联合
│   ├── ScriptNode.tsx
│   ├── StoryboardNode.tsx
│   ├── CharacterNode.tsx
│   ├── TaskNode.tsx
│   ├── AssetNode.tsx
│   ├── NodeContextMenu.tsx             # 节点右键菜单（删除 / 详情）
│   └── shared/
│       ├── NodeShell.tsx               # 通用外壳：标题栏 + body + 状态色边框
│       └── StatusDot.tsx
├── edges/                              # 新建子目录
│   ├── index.ts                        # export edgeTypes（覆盖 spec-21 占位）
│   ├── types.ts                        # EdgeKind 类型联合
│   ├── CharacterToShotEdge.tsx         # 实线 + 角色图标
│   ├── ShotToTaskEdge.tsx              # 虚线
│   ├── TaskToAssetEdge.tsx             # 点线
│   └── EdgeContextMenu.tsx
├── CanvasInner.tsx                     # spec-21 已建，本 spec 加 onPaneContextMenu
├── PaneContextMenu.tsx                 # 新增：画布空白右键菜单（创建节点 / 适应视图）
└── nodeFactory.ts                      # 新增：根据 NodeKind + 实体 id 构造 React Flow Node 对象

src/components/canvas/nodes/types.ts:
```

### 依赖项变更

无新增 npm / Rust 依赖。

**shadcn/ui 新增**：`context-menu`（节点右键 + 边右键 + 画布空白右键统一用同一个组件）。

```bash
pnpm dlx shadcn@latest add context-menu
```

### 节点 data schema

```ts
// src/components/canvas/nodes/types.ts

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
```

> **设计说明**：每种节点的 `data` 字段只存**实体 id**，不冗余实体内容。实体内容通过 hook 实时拉取——这样实体被改动（如分镜 status 从 draft → ready）后，画布上立即体现，无需手动同步。代价是节点渲染期间会触发 N 个 useQuery；TanStack Query 的缓存层 + `staleTime` 合理设置可以把流量压到极小。
>
> `ScriptNodeData.collapsed` 是唯一的"画布本地状态"——它不属于业务实体，但属于布局，所以存进 node.data，随 layout JSON 持久化。

### 5 种节点组件

#### NodeShell 通用外壳

```tsx
// src/components/canvas/nodes/shared/NodeShell.tsx
interface NodeShellProps {
  title: string;
  statusColor?: "gray" | "blue" | "yellow" | "green" | "red";
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
        selected ? "border-primary" : `border-${statusColor}-300`,
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

#### ScriptNode

```tsx
export function ScriptNode({ id, data, selected }: NodeProps<ScriptNodeData>) {
  const episode = useEpisode(data.episodeId);
  const text = episode.data?.script_text ?? "";
  const preview = text.split("\n").slice(0, data.collapsed ? 3 : Infinity).join("\n");
  const updateCollapsed = useCanvasStore((s) => s.updateNodeData);

  return (
    <NodeShell title={`剧本：${episode.data?.title ?? "..."}`}>
      <pre className="whitespace-pre-wrap text-sm" onDoubleClick={() => updateCollapsed(id, { collapsed: !data.collapsed })}>
        {preview || <span className="text-muted-foreground">（暂无剧本内容）</span>}
      </pre>
    </NodeShell>
  );
}
```

> **设计说明**：ScriptNode 的"展开/折叠"切换是 `updateNodeData` 而非 React 局部 state —— 折叠状态写入 node.data，跟随 layout 持久化。next session 打开画布看到的是上次的展开状态。

#### StoryboardNode

```tsx
export function StoryboardNode({ data, selected }: NodeProps<StoryboardNodeData>) {
  const shot = useShot(data.shotId);
  const statusColor: Record<ShotStatus, "gray" | "blue" | "yellow" | "green"> = {
    draft: "gray", ready: "blue", generating: "yellow", done: "green",
  };
  return (
    <NodeShell
      title={`分镜 #${shot.data?.order_index ?? "?"}`}
      statusColor={shot.data ? statusColor[shot.data.status] : "gray"}
      selected={selected}
    >
      <p className="text-sm">{shot.data?.summary || <em className="text-muted-foreground">无摘要</em>}</p>
      {shot.data?.dialogue ? <p className="mt-2 text-xs text-muted-foreground">「{shot.data.dialogue}」</p> : null}
      {shot.data?.duration_sec ? <p className="mt-1 text-xs">{shot.data.duration_sec.toFixed(1)}s</p> : null}
    </NodeShell>
  );
}
```

#### CharacterNode

```tsx
export function CharacterNode({ data, selected }: NodeProps<CharacterNodeData>) {
  const character = useCharacter(data.characterId);
  const refUrl = useResolvedAssetUrl(character.data?.reference_image_path ?? null);
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
export function TaskNode({ data, selected }: NodeProps<TaskNodeData>) {
  const task = useTask(data.taskId); // ← MS2 已有 useTaskById，本 spec 给它加 staleTime: 0 + invalidate on event
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

> **设计说明**：`useTask` 用 MS2 已有的 `useTaskById`，无须改造——MS2 实现的 `useTaskStatusListener` 已经在 event 触发时 invalidateQueries(taskKeys.all)，TaskNode 会自动重渲染。

#### AssetNode

```tsx
export function AssetNode({ data, selected }: NodeProps<AssetNodeData>) {
  const asset = useAsset(data.assetId);
  const thumbUrl = useResolvedAssetUrl(asset.data?.thumbnail_path ?? asset.data?.file_path ?? null);
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
export type EdgeKind = "character_to_shot" | "shot_to_task" | "task_to_asset";
```

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

### 节点工厂 + 上下文菜单

```ts
// src/components/canvas/nodeFactory.ts

export function createScriptNode(episodeId: string, position: XYPosition): Node<ScriptNodeData> {
  return {
    id: `script-${crypto.randomUUID()}`,
    type: "script",
    position,
    data: { kind: "script", episodeId, collapsed: true },
  };
}

export function createStoryboardNode(shotId: string, position: XYPosition): Node<StoryboardNodeData> {
  return {
    id: `storyboard-${shotId}`, // 用 shotId 让同一个 shot 在画布上唯一
    type: "storyboard",
    position,
    data: { kind: "storyboard", shotId },
  };
}

// createCharacter / createTask / createAsset 同款
```

```tsx
// PaneContextMenu.tsx
export function PaneContextMenu({ x, y, paneX, paneY, onClose }: PaneContextMenuProps) {
  const { projectId, episodeId } = useParams();
  const addNode = useCanvasStore((s) => s.addNode);

  return (
    <div className="absolute z-50 rounded-md border bg-popover p-1 shadow-md" style={{ left: x, top: y }}>
      <MenuItem onClick={() => { addNode(createScriptNode(episodeId!, { x: paneX, y: paneY })); onClose(); }}>
        新建剧本节点
      </MenuItem>
      <MenuItem onClick={() => openShotPicker((shotId) => addNode(createStoryboardNode(shotId, { x: paneX, y: paneY })))}>
        添加分镜节点
      </MenuItem>
      <MenuItem onClick={() => openCharacterPicker((charId) => addNode(createCharacterNode(charId, { x: paneX, y: paneY })))}>
        添加角色节点
      </MenuItem>
      <MenuItem onClick={() => openTaskPicker((taskId) => addNode(createTaskNode(taskId, { x: paneX, y: paneY })))}>
        添加任务节点
      </MenuItem>
      <MenuItem onClick={() => openAssetPicker((assetId) => addNode(createAssetNode(assetId, { x: paneX, y: paneY })))}>
        添加素材节点
      </MenuItem>
    </div>
  );
}
```

> **设计说明**：用户在画布空白右键 → 选「添加分镜节点」→ 弹 ShotPickerDialog（复用 spec-20 实现的 ShotListPanel 的子组件）。这避免「画布上 N 个新分镜节点漂浮但没绑 shot」的孤儿态——节点和实体 1:1 绑定。

### 连线行为

React Flow 12 的 `onConnect` 在 spec-21 的 store 已实现 `addEdge`。本 spec 在 `onConnect` 上叠加业务规则：

```ts
// canvasStore.ts 增强 onConnect
onConnect: async (params) => {
  const sourceNode = get().nodes.find((n) => n.id === params.source);
  const targetNode = get().nodes.find((n) => n.id === params.target);
  if (!sourceNode || !targetNode) return;

  const edgeKind = inferEdgeKind(sourceNode.data, targetNode.data);
  if (!edgeKind) {
    toast.error(`不支持 ${sourceNode.data.kind} → ${targetNode.data.kind} 的连线`);
    return;
  }

  set({ edges: addEdge({ ...params, type: edgeKind }, get().edges) });

  // 业务副作用：角色→分镜时写 shot_character 关联表
  if (edgeKind === "character_to_shot") {
    const { characterId } = sourceNode.data as CharacterNodeData;
    const { shotId } = targetNode.data as StoryboardNodeData;
    try {
      await commands.linkShotSubject(shotId, characterId, "character");
    } catch (e) {
      toast.error(`绑定角色到分镜失败：${e}`);
      // 回滚 edge
      set({ edges: get().edges.filter((e) => e.id !== params.id) });
    }
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

### 节点删除

`canvasStore.onNodesChange` 已被 spec-21 实现，本 spec 增强：

```ts
onNodesChange: (changes) => {
  // 拦截 'remove' 变化：如果删除的是 character→shot edge 关联的节点，反向 unlink
  for (const change of changes) {
    if (change.type === "remove") {
      const node = get().nodes.find((n) => n.id === change.id);
      if (node?.data.kind === "character") {
        // 找到所有以该 character 为 source 的 character_to_shot edges
        const affectedEdges = get().edges.filter(
          (e) => e.source === node.id && e.type === "character_to_shot"
        );
        for (const e of affectedEdges) {
          const shotNode = get().nodes.find((n) => n.id === e.target);
          if (shotNode?.data.kind === "storyboard") {
            commands.unlinkShotSubject(shotNode.data.shotId, node.data.characterId, "character")
              .catch(console.error);
          }
        }
      }
    }
  }
  set({ nodes: applyNodeChanges(changes, get().nodes) });
},
```

### 错误场景

| 场景 | 行为 |
|---|---|
| 节点 data 引用的实体已被删除（如 shot_id 对应的 shot 被删） | useShot 返回 NotFound；StoryboardNode 显示「[分镜已删除]」 + 一个「从画布移除」按钮 |
| 连线源/目标类型不被支持（如 asset → character） | toast「不支持的连线类型」，不写入 edges |
| `link_shot_subject` IPC 失败 | toast 错误 + 回滚刚加的 edge |
| 节点 data schema 不认识（如未来引入新 NodeKind 后回滚） | nodeTypes registry 命中失败，React Flow 渲染 `default` 节点显示 raw data；不崩溃 |
| 右键菜单点击「添加分镜节点」但项目下没有 shot | ShotPickerDialog 显示空态 + 「新建一个分镜」按钮（跳 EpisodeDetailPage） |
| StoryboardNode 重复创建（用户两次添加同一 shot） | createStoryboardNode 用 `storyboard-${shotId}` 作为节点 id，第二次 addNode 时 React Flow 自动拒绝（id 唯一） + toast「该分镜已在画布上」 |

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
- createScriptNode 的 id 含 uuid
- 所有 factory 返回的 node.data.kind 与 NodeKind 一致

`src/stores/canvasStore.connect.test.ts`：
- inferEdgeKind 三种合法组合返回对应 kind
- 非法组合返回 null
- character→storyboard 连线触发 linkShotSubject mutation
- mutation 失败时 edge 被回滚

#### 手动验证清单

- [ ] 画布空白右键 → 看到 5 个「添加节点」菜单项
- [ ] 添加一个 ScriptNode → 看到 episode.script_text 前 3 行
- [ ] 双击 ScriptNode → 展开看到全文 → 刷新页面 → 仍保持展开
- [ ] 添加一个 StoryboardNode（选 shot status=draft）→ 看到灰色边框
- [ ] 在 EpisodeDetailPage 改 shot status 为 ready → 回画布 → StoryboardNode 边框变蓝
- [ ] 添加 CharacterNode + StoryboardNode → 拖连线 → 看到实线 + 角色图标 → DB 查 `shot_character` 有新行
- [ ] 删除 CharacterNode → DB 查 `shot_character` 对应行消失
- [ ] 添加 TaskNode（选一个 running 状态的任务） → 看到黄色边框 → 任务完成 → 边框变绿
- [ ] 添加 AssetNode（选 video） → 看到「播放」按钮 → 点击弹 ResultPreviewDialog（复用 MS2 已有）

## 验收标准

- [ ] 5 种节点组件全部实现，传 `data.kind` 字段做 discriminated union
- [ ] 3 种 Edge 样式可视化区分（线型 + 图标）
- [ ] `nodeTypes` / `edgeTypes` registry 正确注册
- [ ] PaneContextMenu + NodeContextMenu + EdgeContextMenu 三个右键菜单工作
- [ ] CharacterToShotEdge 连线同步写 `shot_character` 表
- [ ] 删除连线 / 删除节点反向 unlink
- [ ] 引用实体被删时节点显示空态而非崩溃
- [ ] `pnpm test:run` 至少新增 15 个前端测试
- [ ] `pnpm typecheck` 干净
- [ ] 手动验证清单全过

## 依赖关系

- **前置**：spec-20（episode / shot / character / link_shot_subject 全部 IPC）+ spec-21（CanvasInner / canvasStore / nodeTypes registry）
- **平行**：spec-23（拖放）—— 拖放产生节点的 factory 复用本 spec 的 `nodeFactory.ts`，因此**建议 spec-22 先合**
- **后续**：
  - SPEC-24 Undo/Redo 必须覆盖节点 data 的变化（`updateNodeData` 触发 zundo snapshot）

## 与原任务文档的差异

合并自原任务 2（ScriptNode）+ 任务 3（StoryboardNode）+ 任务 4（CharacterNode）+ 任务 5（TaskNode）+ 任务 6（AssetNode）+ 任务 7（自定义 Edge）。

- **原任务 4 提到"连线：可连接到 StoryboardNode"但没说怎么写 shot_character 关联表**：本 spec 明确"画布 edge 与 shot_character 双写"的策略与回滚机制
- **原任务 8 提到"拖放到分镜节点上：绑定为该分镜的参考图/采用视频"**：拖放本身由 spec-23 实现，但**绑定语义**（asset → shot 的关联）属于本 spec 的 EdgeKind 范畴。本 spec 暂不实现"asset_to_shot"边，因为 `asset.shot_id` 已经是 DB 列（不是关联表），由 spec-23 拖放时直接 update_asset 处理；画布上的视觉表达留给 spec-23 时再决定（是新增 EdgeKind 还是把 asset 节点画在 shot 节点旁）
- **原任务 5 提到"实时状态更新：监听 Tauri event"**：复用 MS2 已建的 `useTaskStatusListener` + invalidateQueries 机制，本 spec 不重复造轮子
