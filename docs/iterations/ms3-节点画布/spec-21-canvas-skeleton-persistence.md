# SPEC-21: 画布骨架 + CanvasLayout 持久化

## 概述

集成 `@xyflow/react`（React Flow 12），搭建画布容器、基础交互（缩放/平移/minimap/controls/background），并打通「按 episode 加载布局 → 用户操作 → 防抖 1s 自动保存到 `canvas_layout` 表」的最小持久化闭环。

本 spec **不实现任何业务节点**——画布上能拖出/连接的只有 React Flow 内置 `default` 节点，仅用于验证持久化。5 种业务节点由 spec-22 实现；本 spec 提供节点工厂的注册点。

> ⚠️ **破坏性变更**：`canvas_layout` 表 IPC 暴露给前端（之前 0 个 IPC）；`WorkspaceNav` 上 spec-20 的「跳画布」按钮启用；引入 `@xyflow/react` 12.x、`zundo`、`use-debounce` 三个 npm 依赖。

## 设计目标 / 非目标

**目标**：

- `canvas_layout` 表的 model / queries / 3 个 Tauri commands（get_by_episode / upsert / delete_by_episode）
- 前端 `<CanvasContainer episodeId={...} />` 组件挂载 React Flow，渲染 background grid + minimap + controls
- 「打开 episode → load layout JSON → 反序列化为 nodes/edges/viewport → 初始化 ReactFlow」加载链路
- 「nodes/edges 变化 → 防抖 1s → upsert 到 DB」自动保存链路
- 「viewport 拖动/缩放 → onMoveEnd → 同上 upsert」
- 节点工厂注册点：`src/components/canvas/nodeTypes.ts` 维护 `Record<NodeKind, ReactFlowNodeType>`，spec-22 往里塞 5 种业务节点
- 路由：`/project/:projectId/episodes/:episodeId/canvas` 进入画布
- 增量保存优化：保存前对当前 layout JSON 求哈希，与上次保存的哈希对比，未变更则跳过 DB 写入（防止 viewport 微抖动触发的无意义写入）

**非目标**（明确划归后续 spec）：

- 5 种业务节点（Script/Storyboard/Character/Task/Asset）→ spec-22
- 自定义 Edge 样式 → spec-22
- 拖放（内部面板 + 系统文件）→ spec-23
- Undo/Redo → spec-24
- 工具栏（缩放按钮、适应视图、保存版本）→ spec-24
- 大画布 Web Worker 序列化 → 本 spec 仅用 `requestIdleCallback`，Worker 留给后续优化（见 §"性能优化决策树"）
- 多 episode 同时打开（tab 切换） → MVP 单 episode 即可

## 技术方案

### 目录/文件结构

```text
crates/core/src/
├── models/
│   ├── mod.rs                          # +pub mod canvas_layout;
│   └── canvas_layout.rs                # 新增：CanvasLayout + UpsertCanvasLayoutInput
└── db/queries/
    ├── mod.rs                          # +pub mod canvas_layout;
    └── canvas_layout.rs                # get_by_episode / upsert / delete_by_episode

src-tauri/src/
├── lib.rs                              # collect_commands! 增加 3 项
└── commands/
    ├── mod.rs                          # +pub mod canvas_layout;
    └── canvas_layout.rs                # 3 个 IPC

src/
├── routes.tsx                          # /episodes/:episodeId/canvas 路由
├── pages/
│   └── CanvasPage.tsx                  # 新增：路由入口
├── components/canvas/                  # 新建子目录
│   ├── CanvasContainer.tsx             # ReactFlowProvider + 加载/保存逻辑
│   ├── CanvasInner.tsx                 # 实际渲染 ReactFlow
│   ├── nodeTypes.ts                    # spec-22 节点工厂注册点（本 spec 仅占位 {}）
│   ├── edgeTypes.ts                    # spec-22 边工厂注册点（本 spec 仅占位 {}）
│   └── persistence/
│       ├── serialization.ts            # 序列化 / 反序列化 + 哈希
│       └── useAutoSave.ts              # 防抖保存 hook
├── stores/
│   └── canvasStore.ts                  # Zustand store（spec-24 加 zundo middleware）
└── hooks/
    └── useCanvasLayout.ts              # useCanvasLayoutQuery / useCanvasLayoutMutation
```

### 依赖项变更

**npm 新增**（详见 `dependencies.md`）：

```json
{
  "dependencies": {
    "@xyflow/react": "^12.x.x",
    "zundo": "^2.x.x",
    "use-debounce": "^10.x.x"
  }
}
```

> `zundo` 在本 spec 暂不引用，预声明用于 spec-24。把它放在 spec-21 的 `package.json` 增量里是为了让 PR1（spec-20）→ PR2（spec-21）依赖增量集中评审；spec-24 实施时只引用，不再 `pnpm add`。

**Rust**：无新增。
**Tauri 插件**：无新增。

**shadcn/ui**：本 spec 不新增（tooltip / context-menu 留给 spec-24）。

### 数据模型

`crates/core/src/models/canvas_layout.rs`：

```rust
use serde::{Deserialize, Serialize};
use specta::Type;

/// 画布布局。`nodes_json` / `edges_json` / `viewport_json` 是不透明的 JSON 字符串：
/// core 不解析其内部结构（节点 schema 由前端 React Flow 拥有），只保证是合法 JSON。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CanvasLayout {
    pub id: String,
    pub episode_id: String,
    pub nodes_json: String,
    pub edges_json: String,
    pub viewport_json: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Type)]
pub struct UpsertCanvasLayoutInput {
    pub episode_id: String,
    pub nodes_json: String,
    pub edges_json: String,
    pub viewport_json: String,
}
```

> **设计说明**：把 nodes/edges/viewport **不**拆三列字段调用、不在 Rust 端建强类型，是有意为之。React Flow 12 的节点 schema 还在演进（自定义节点的 `data` 字段、`measured` 字段等），如果 Rust 端建立 mirror 类型，前后端会反复同步。代价是：Rust 端无法基于节点内容做查询（如"找出所有引用了 asset X 的画布"）；这种查询当前没有需求。当出现需求时再加一张 `canvas_node_index` 派生表，**不动主存储格式**。

### Rust 查询层

`crates/core/src/db/queries/canvas_layout.rs`：

```rust
const SELECT_COLUMNS: &str = "id, episode_id, nodes_json, edges_json, viewport_json, updated_at";

pub fn get_by_episode(conn: &Connection, episode_id: &str) -> Result<Option<CanvasLayout>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM canvas_layout WHERE episode_id = ?1"
    ))?;
    let row = stmt.query_row(params![episode_id], row_to_canvas_layout).optional()?;
    Ok(row)
}

pub fn upsert(conn: &Connection, input: &UpsertCanvasLayoutInput) -> Result<CanvasLayout> {
    // 合法 JSON 校验：core 不解析结构，但拒绝写入垃圾字节
    validate_json(&input.nodes_json, "nodes_json")?;
    validate_json(&input.edges_json, "edges_json")?;
    validate_json(&input.viewport_json, "viewport_json")?;

    conn.execute(
        "INSERT INTO canvas_layout (id, episode_id, nodes_json, edges_json, viewport_json, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))
         ON CONFLICT(episode_id) DO UPDATE SET
            nodes_json = excluded.nodes_json,
            edges_json = excluded.edges_json,
            viewport_json = excluded.viewport_json,
            updated_at = datetime('now')",
        params![
            uuid::Uuid::new_v4().to_string(),
            input.episode_id,
            input.nodes_json,
            input.edges_json,
            input.viewport_json,
        ],
    )?;
    get_by_episode(conn, &input.episode_id)?.ok_or_else(|| {
        CoreError::NotFound { entity: "canvas_layout", id: input.episode_id.clone() }
    })
}

pub fn delete_by_episode(conn: &Connection, episode_id: &str) -> Result<()> {
    conn.execute("DELETE FROM canvas_layout WHERE episode_id = ?1", params![episode_id])?;
    Ok(())
}

fn validate_json(s: &str, field: &'static str) -> Result<()> {
    serde_json::from_str::<serde_json::Value>(s).map_err(|e| {
        CoreError::Validation(format!("{field} is not valid JSON: {e}").into())
    })?;
    Ok(())
}
```

> **设计说明**：用 `001_initial.sql` 已建的 `UNIQUE INDEX idx_canvas_episode ON canvas_layout(episode_id)` 触发 `ON CONFLICT`，实现 episode 维度的 upsert。**不**给 `id` 用 episode_id（保留 uuid pk）—— 这样 episode_id 可以未来切换（如克隆 episode）而不破 pk 唯一性。

### Tauri commands

`src-tauri/src/commands/canvas_layout.rs`：

```rust
#[tauri::command] #[specta::specta]
pub async fn get_canvas_layout(state: State<'_, AppState>, episode_id: String) -> Result<Option<CanvasLayout>, IpcError> {
    state.db.call(move |c| canvas_q::get_by_episode(c, &episode_id).map_err(Into::into)).await.map_err(IpcError::from)
}

#[tauri::command] #[specta::specta]
pub async fn upsert_canvas_layout(state: State<'_, AppState>, input: UpsertCanvasLayoutInput) -> Result<CanvasLayout, IpcError> { /* ... */ }

#[tauri::command] #[specta::specta]
pub async fn delete_canvas_layout(state: State<'_, AppState>, episode_id: String) -> Result<(), IpcError> { /* ... */ }
```

### 前端

#### 路由

```tsx
// src/routes.tsx, episodes 子树扩展
{
  path: ":episodeId",
  element: <EpisodeDetailPage />,
  children: [
    { path: "canvas", element: <CanvasPage /> },
  ],
},
```

EpisodeDetailPage 的「跳画布」按钮去掉 disabled，链接到 `canvas` 子路由。

#### CanvasPage

```tsx
export default function CanvasPage() {
  const { episodeId } = useParams<{ episodeId: string }>();
  if (!episodeId) return <Navigate to=".." replace />;
  return (
    <ReactFlowProvider>
      <CanvasContainer episodeId={episodeId} />
    </ReactFlowProvider>
  );
}
```

#### CanvasContainer：加载 + 提供 store

```tsx
export function CanvasContainer({ episodeId }: { episodeId: string }) {
  const layoutQuery = useCanvasLayoutQuery(episodeId);
  const initStore = useCanvasStore((s) => s.init);

  useEffect(() => {
    if (layoutQuery.data === undefined) return; // loading
    const { nodes, edges, viewport } = deserializeLayout(layoutQuery.data);
    initStore({ nodes, edges, viewport });
  }, [layoutQuery.data, initStore]);

  if (layoutQuery.isLoading) return <Skeleton />;
  return <CanvasInner episodeId={episodeId} />;
}
```

#### CanvasInner：渲染 + 自动保存

```tsx
export function CanvasInner({ episodeId }: { episodeId: string }) {
  const nodes = useCanvasStore((s) => s.nodes);
  const edges = useCanvasStore((s) => s.edges);
  const onNodesChange = useCanvasStore((s) => s.onNodesChange);
  const onEdgesChange = useCanvasStore((s) => s.onEdgesChange);
  const onConnect = useCanvasStore((s) => s.onConnect);

  useAutoSaveLayout(episodeId);

  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      onNodesChange={onNodesChange}
      onEdgesChange={onEdgesChange}
      onConnect={onConnect}
      nodeTypes={nodeTypes}
      edgeTypes={edgeTypes}
      defaultViewport={useCanvasStore.getState().viewport}
      onMoveEnd={(_, viewport) => useCanvasStore.getState().setViewport(viewport)}
      fitView={false}
      minZoom={0.1}
      maxZoom={2}
    >
      <Background gap={20} />
      <MiniMap pannable zoomable />
      <Controls />
    </ReactFlow>
  );
}
```

#### Zustand store

`src/stores/canvasStore.ts`：

```ts
import { create } from "zustand";
import {
  applyNodeChanges, applyEdgeChanges, addEdge,
  type Node, type Edge, type Viewport,
  type OnNodesChange, type OnEdgesChange, type OnConnect,
} from "@xyflow/react";

interface CanvasState {
  nodes: Node[];
  edges: Edge[];
  viewport: Viewport;
  init: (s: { nodes: Node[]; edges: Edge[]; viewport: Viewport }) => void;
  onNodesChange: OnNodesChange;
  onEdgesChange: OnEdgesChange;
  onConnect: OnConnect;
  setViewport: (v: Viewport) => void;
}

export const useCanvasStore = create<CanvasState>((set, get) => ({
  nodes: [],
  edges: [],
  viewport: { x: 0, y: 0, zoom: 1 },
  init: (s) => set(s),
  onNodesChange: (changes) => set({ nodes: applyNodeChanges(changes, get().nodes) }),
  onEdgesChange: (changes) => set({ edges: applyEdgeChanges(changes, get().edges) }),
  onConnect: (params) => set({ edges: addEdge(params, get().edges) }),
  setViewport: (v) => set({ viewport: v }),
}));
```

> **设计说明**：store 暴露 `init` 而非构造函数式注入——因为 React Flow 的 `ReactFlowProvider` 必须先挂载，store 在 mount 后由 `CanvasContainer` 异步注入。zundo middleware 由 spec-24 套上，本 spec 用纯净 store。

#### 自动保存 hook

`src/components/canvas/persistence/useAutoSave.ts`：

```ts
import { useEffect, useRef } from "react";
import { useDebouncedCallback } from "use-debounce";
import { useCanvasStore } from "@/stores/canvasStore";
import { useCanvasLayoutMutation } from "@/hooks/useCanvasLayout";
import { serializeLayout, hashLayout } from "./serialization";

const AUTO_SAVE_DEBOUNCE_MS = 1000;

export function useAutoSaveLayout(episodeId: string) {
  const lastHashRef = useRef<string | null>(null);
  const mutation = useCanvasLayoutMutation();

  const save = useDebouncedCallback(() => {
    const { nodes, edges, viewport } = useCanvasStore.getState();
    const payload = serializeLayout({ nodes, edges, viewport });
    const hash = hashLayout(payload);
    if (hash === lastHashRef.current) return; // 跳过无变更写入
    lastHashRef.current = hash;
    mutation.mutate({ episode_id: episodeId, ...payload });
  }, AUTO_SAVE_DEBOUNCE_MS);

  useEffect(() => {
    const unsubscribe = useCanvasStore.subscribe(save);
    return () => unsubscribe();
  }, [save]);
}
```

> **设计说明**：保存触发器用 `useCanvasStore.subscribe(callback)` —— 所有状态变化（节点移动、连线、viewport）都会触发；防抖确保用户连续操作只产生一次写入。哈希对比是**跳过无变更写入**的关键，避免 React Flow 内部偶尔触发的"等值变化"也写库。

#### 序列化与哈希

`src/components/canvas/persistence/serialization.ts`：

```ts
import type { Node, Edge, Viewport } from "@xyflow/react";

interface RawLayout {
  nodes: Node[];
  edges: Edge[];
  viewport: Viewport;
}

interface SerializedLayout {
  nodes_json: string;
  edges_json: string;
  viewport_json: string;
}

export function serializeLayout(raw: RawLayout): SerializedLayout {
  return {
    nodes_json: JSON.stringify(raw.nodes),
    edges_json: JSON.stringify(raw.edges),
    viewport_json: JSON.stringify(raw.viewport),
  };
}

export function deserializeLayout(stored: CanvasLayout | null): RawLayout {
  if (!stored) return { nodes: [], edges: [], viewport: { x: 0, y: 0, zoom: 1 } };
  return {
    nodes: JSON.parse(stored.nodes_json) as Node[],
    edges: JSON.parse(stored.edges_json) as Edge[],
    viewport: JSON.parse(stored.viewport_json) as Viewport,
  };
}

/// 简易 djb2 哈希；不需要密码学强度，只要"内容相同 → 哈希相同"
export function hashLayout(s: SerializedLayout): string {
  return djb2(s.nodes_json) + ":" + djb2(s.edges_json) + ":" + djb2(s.viewport_json);
}

function djb2(s: string): string {
  let h = 5381;
  for (let i = 0; i < s.length; i++) h = ((h << 5) + h) + s.charCodeAt(i);
  return (h >>> 0).toString(36);
}
```

#### TanStack Query hook

`src/hooks/useCanvasLayout.ts`：

```ts
const canvasLayoutKey = (episodeId: string) => ["canvas_layout", episodeId] as const;

export function useCanvasLayoutQuery(episodeId: string) {
  return useQuery({
    queryKey: canvasLayoutKey(episodeId),
    queryFn: () => commands.getCanvasLayout(episodeId),
    staleTime: Infinity, // 自动保存自己管 invalidate
  });
}

export function useCanvasLayoutMutation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: UpsertCanvasLayoutInput) => commands.upsertCanvasLayout(input),
    onSuccess: (data) => {
      qc.setQueryData(canvasLayoutKey(data.episode_id), data);
    },
  });
}
```

### 性能优化决策树

| 节点数 | 序列化策略 | 触发时机 |
|---|---|---|
| < 30 | `JSON.stringify` 主线程同步 | debounce 触发 |
| 30 ~ 80 | `requestIdleCallback` 在空闲帧调 stringify | debounce 触发 + 空闲帧 |
| > 80 | Web Worker（`comlink` 或裸 Worker） | 后续 spec 单独评估 |

**本 spec 实现「< 30」与「30~80」两档**——`useAutoSave` 在 debounce 触发后通过 `if ('requestIdleCallback' in window) { requestIdleCallback(serialize) } else { serialize() }` 自动选择。Web Worker 升级触发条件：`pnpm tauri dev` 实测 100 节点画布拖动 FPS < 30。

### 错误场景

| 场景 | 行为 |
|---|---|
| `get_canvas_layout` 时 episode 不存在 | 返回 `Ok(None)`，前端按"新建空画布"处理 |
| `upsert` 时 episode 不存在 | FK 触发 `CoreError::Sqlite`；IPC 返回 IpcError，前端 toast「该集已被删除」+ 路由跳回 EpisodeListPage |
| `upsert` 时 nodes_json 不是合法 JSON | core 层 `validate_json` 拒绝；前端不应能触发（自家序列化的），出现即 bug |
| React Flow 内部抛错（如自定义节点 render 报错） | React error boundary 捕获（CanvasPage 顶层挂一个 `<CanvasErrorBoundary>`），显示「画布渲染异常，请联系开发者」+ 「回滚到上次保存」按钮 |
| 多次 mutation in-flight（用户高速操作） | TanStack Query 默认串行化同 key 的 mutation；后到的覆盖前面的（"last write wins"） |
| 序列化时 layout 包含 circular reference | `JSON.stringify` 抛 TypeError；hook 内 try/catch，console.error + 跳过该次保存（保护用户数据不被清空） |
| 浏览器关闭/路由切换时还有 pending 防抖 | `useAutoSave` cleanup 中 `flush()` 强制立即保存。详见 `use-debounce` 的 `flush` API |

### 测试策略

#### Rust 单元测试

`crates/core/src/db/queries/canvas_layout.rs`：
- `get_by_episode` 空时返回 None
- `upsert` 首次插入（INSERT 路径） → 第二次同 episode_id（UPDATE 路径） → updated_at 推进
- `upsert` 时 nodes_json 是 `"{"` → 返回 Validation 错误，不写库
- `delete_by_episode` 后 get 返回 None
- episode 被删除时 canvas_layout 级联删除（依赖 schema FK，但加测试覆盖回归）

#### 前端测试

`src/components/canvas/persistence/serialization.test.ts`：
- `serializeLayout` + `deserializeLayout` 完整往返
- `hashLayout` 相同输入 → 相同输出；nodes 顺序不同 → 哈希不同
- `deserializeLayout(null)` 返回空 nodes/edges + 默认 viewport

`src/components/canvas/persistence/useAutoSave.test.ts`：
- mock `useCanvasStore` + `useCanvasLayoutMutation`
- 触发 store 变化 → 1100ms 后 mutation 被调用一次
- 触发两次相同内容变化（如先加节点再删回去）→ 哈希一致 → mutation 仅调用一次

#### 手动验证清单

- [ ] `pnpm tauri dev` 进项目 → 「剧本」 → 进入一集 → 点「跳画布」→ 看到空画布 + minimap + controls + grid
- [ ] 在画布上双击空白处（暂不会创建节点，但不应报错）
- [ ] 缩放滚轮 / 拖动平移正常
- [ ] 用 Tauri DevTools 调 `invoke("upsert_canvas_layout", {input:{episode_id:"...", nodes_json:"[{\"id\":\"1\",\"position\":{\"x\":0,\"y\":0},\"data\":{\"label\":\"test\"}}]", edges_json:"[]", viewport_json:"{}"}})` → 刷新页面 → 看到节点恢复
- [ ] DB 直查 `SELECT * FROM canvas_layout WHERE episode_id = '...'` 验证字段写入
- [ ] 反复拖动节点 → DB updated_at 1s 一次而非每帧

## 验收标准

- [ ] `crates/core/src/models/canvas_layout.rs` + `db/queries/canvas_layout.rs` 完整实现
- [ ] 3 个 Tauri commands 注册（get / upsert / delete_by_episode）
- [ ] `package.json` 增加 `@xyflow/react` / `zundo` / `use-debounce` 三依赖，版本已通过 web 验证
- [ ] `src/routes.tsx` 新增 `episodes/:episodeId/canvas` 路由
- [ ] EpisodeDetailPage 的「跳画布」按钮启用
- [ ] CanvasContainer / CanvasInner / canvasStore / useAutoSaveLayout 完整实现
- [ ] 自动保存防抖 1s，哈希去重生效
- [ ] viewport 持久化（关闭重开缩放/位置恢复）
- [ ] `cargo test -p mango-core` 新增至少 5 个 canvas_layout 测试全绿
- [ ] `cargo clippy -p mango-core --all-targets -D warnings` 干净
- [ ] `pnpm typecheck` 干净
- [ ] `pnpm test:run` 至少新增 6 个前端测试（serialization 3 + useAutoSave 3）
- [ ] `cargo test -p mango-tauri export_bindings` 重新生成 commands.ts diff 仅含 3 个新命令
- [ ] 手动验证清单全过

## 依赖关系

- **前置**：spec-20（episode 表的 CRUD + EpisodeDetailPage 才能跳到画布）
- **平行**：无
- **后续**：
  - SPEC-22 把业务节点塞进 `nodeTypes` / `edgeTypes` registry
  - SPEC-23 在 `CanvasInner` 上挂 `onDrop` / Tauri `onDragDropEvent` 监听器
  - SPEC-24 给 `canvasStore` 套 `zundo` middleware + 工具栏组件

## 与原任务文档的差异

合并自原任务 1（React Flow 集成）+ 任务 10（CanvasLayout 持久化）。

- **原任务 10 提到 Web Worker 序列化**：本 spec 实现 `requestIdleCallback` 兜底但**不引入 Worker**，理由见 §"性能优化决策树"。Worker 升级触发条件明确写入 spec
- **原任务 1 没提"按 episode 加载"**：本 spec 把"episode_id 是 url 路由参数 + queryKey + 持久化主键"这条线明确写出，让 spec-22 的节点知道自己挂在哪个 episode 下
- **原任务文档没提哈希去重**：本 spec 引入，理由是 React Flow 内部偶尔会触发等值变化（如 onMoveEnd 重复 emit 同一 viewport），不去重会污染 DB 写入频率
