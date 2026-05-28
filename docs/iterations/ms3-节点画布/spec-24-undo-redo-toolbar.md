# SPEC-24: Undo/Redo + 画布工具栏 + EpisodeCheckpoint 占位

## 概述

给 spec-21 建立的 `canvasStore` 套上 `zundo` temporal middleware，实现 Ctrl+Z / Ctrl+Y / Cmd+Shift+Z 撤销重做；在画布顶部叠加 `<CanvasToolbar>`，提供缩放 / 适应视图 / 撤销 / 重做 / 保存版本五个工具按钮。

「保存版本」按钮在本 spec 中**仅写入一条最小化 `episode_checkpoint` 行**作为占位——完整的版本管理（恢复 / 列表 / 自动清理 / SQLite backup）属于 MS4 范畴。本 spec 保证按钮和 IPC 命令可点击、不报错、DB 有写入痕迹，让 MS4 可以无缝接续。

> ⚠️ **破坏性变更**：`canvasStore` 引入 `temporal` middleware，store 形状不变但调用方式新增 `useCanvasStore.temporal.getState()`；`canvas_layout` 之外新增 `episode_checkpoint` 表的 1 个 Tauri command（create_checkpoint，占位实现）。

## 设计目标 / 非目标

**目标**：

- `canvasStore` 套 `zundo` middleware，`partialize` 仅追踪 `{ nodes, edges }`，**不追踪** viewport（避免拖动画布污染 undo 栈）
- 栈深度 50，配置 `equality` 函数避免 React Flow 内部偶发的同值变化触发 snapshot
- 快捷键：Ctrl+Z / Cmd+Z 撤销；Ctrl+Y / Ctrl+Shift+Z / Cmd+Shift+Z 重做。系统级 textarea/input focus 时**不**拦截
- `<CanvasToolbar>` 五个按钮 + 缩放百分比指示器，置于画布左上角；shadcn `tooltip` 给每个按钮 hover 提示
- 「保存版本」按钮 → 调 `create_episode_checkpoint(episode_id, note?)` → 写一行 `episode_checkpoint` （仅 id + episode_id + canvas_layout_snapshot + created_at + note）→ toast 「已保存版本」
- 「适应视图」用 React Flow 12 `useReactFlow().fitView({ padding: 0.2, duration: 300 })`
- `episode_checkpoint` Rust model + 1 个 query `insert_minimal` + 1 个 IPC

**非目标**（明确划归后续 spec / MS）：

- EpisodeCheckpoint 列表 UI / 恢复操作 / 自动清理策略 → MS4
- SQLite backup（`VACUUM INTO`）→ MS4
- Undo/Redo 跨业务副作用（如撤销 character→shot 连线时反向重建 `shot_character` 行）→ **本 spec 决议「只撤销画布层 nodes/edges，业务表副作用一次性写入，撤销不回放」**；详见 §"Undo 与业务副作用的边界"
- 工具栏更多按钮（导出 / 截图 / 锁定视图）→ V2
- 触屏 / 手势缩放 → V2

## 技术方案

### 目录/文件结构

```text
crates/core/src/
├── models/
│   ├── mod.rs                          # +pub mod episode_checkpoint;
│   └── episode_checkpoint.rs           # 新增：EpisodeCheckpoint + CreateCheckpointInput（最小化）
└── db/queries/
    ├── mod.rs                          # +pub mod episode_checkpoint;
    └── episode_checkpoint.rs           # 新增：insert_minimal（仅写一行，留 MS4 扩展）

src-tauri/src/
├── lib.rs                              # collect_commands! +1 项
└── commands/
    ├── mod.rs                          # +pub mod episode_checkpoint;
    └── episode_checkpoint.rs           # create_episode_checkpoint（占位）

src/
├── components/canvas/
│   ├── CanvasInner.tsx                 # spec-21 已建，本 spec 加 <CanvasToolbar /> 渲染
│   ├── toolbar/
│   │   ├── CanvasToolbar.tsx           # 新增：五按钮 + 缩放指示器
│   │   ├── ZoomIndicator.tsx           # 新增
│   │   └── useCanvasShortcuts.ts       # 新增：全局键盘 hook（Cmd+Z / Cmd+Shift+Z / Cmd+Y）
│   └── persistence/
│       └── useAutoSave.ts              # spec-21 已建，本 spec 加 zundo subscribe 衔接
└── stores/
    └── canvasStore.ts                  # spec-21 已建，本 spec 套 temporal middleware + partialize
```

### 依赖项变更

**npm 新增**：无（`zundo` 已在 spec-21 的 package.json 增量中预声明）

**shadcn/ui 新增**：`tooltip`（如 spec-22 已 add 则跳过）

```bash
pnpm dlx shadcn@latest add tooltip
```

**Rust**：无新增。
**Tauri 插件**：无新增。
**Migration**：无（`episode_checkpoint` 表已在 `001_initial.sql`）

### `episode_checkpoint` 表确认

参照 MS0 `001_initial.sql` 已有：

```sql
CREATE TABLE episode_checkpoint (
    id          TEXT PRIMARY KEY,
    episode_id  TEXT NOT NULL REFERENCES episode(id) ON DELETE CASCADE,
    snapshot    TEXT NOT NULL,           -- JSON：canvas_layout 全量序列化
    note        TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_checkpoint_episode ON episode_checkpoint(episode_id, created_at DESC);
```

> 字段名以实际 migration 为准；如 MS0 的列名是 `layout_snapshot` 或 `canvas_layout_json` 等，本 spec 的 model 字段与之对齐，不改 schema。

### Rust 数据模型

`crates/core/src/models/episode_checkpoint.rs`：

```rust
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EpisodeCheckpoint {
    pub id: String,
    pub episode_id: String,
    /// 完整 canvas_layout JSON 快照（nodes + edges + viewport，包成一个 wrapper 对象）
    pub snapshot: String,
    pub note: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Type)]
pub struct CreateCheckpointInput {
    pub episode_id: String,
    /// 用户备注，默认空字符串
    #[serde(default)]
    pub note: Option<String>,
}
```

### Rust 查询层

`crates/core/src/db/queries/episode_checkpoint.rs`：

```rust
use rusqlite::{params, Connection};
use crate::{Result, CoreError};
use crate::models::episode_checkpoint::*;
use crate::db::queries::canvas_layout as canvas_q;

/// 最小化创建：抓取当前 episode 的 canvas_layout 作为 snapshot 写入。
/// 不实现 list / restore / delete —— 那些由 MS4 实施。
pub fn insert_minimal(conn: &Connection, input: &CreateCheckpointInput) -> Result<EpisodeCheckpoint> {
    let layout = canvas_q::get_by_episode(conn, &input.episode_id)?
        .ok_or_else(|| CoreError::NotFound { entity: "canvas_layout", id: input.episode_id.clone() })?;

    let snapshot = serde_json::json!({
        "nodes_json":    layout.nodes_json,
        "edges_json":    layout.edges_json,
        "viewport_json": layout.viewport_json,
    }).to_string();

    let id = uuid::Uuid::new_v4().to_string();
    let note = input.note.as_deref().unwrap_or("");
    conn.execute(
        "INSERT INTO episode_checkpoint (id, episode_id, snapshot, note) VALUES (?1, ?2, ?3, ?4)",
        params![id, input.episode_id, snapshot, note],
    )?;

    let mut stmt = conn.prepare(
        "SELECT id, episode_id, snapshot, note, created_at FROM episode_checkpoint WHERE id = ?1"
    )?;
    stmt.query_row(params![id], |r| Ok(EpisodeCheckpoint {
        id: r.get(0)?,
        episode_id: r.get(1)?,
        snapshot: r.get(2)?,
        note: r.get(3)?,
        created_at: r.get(4)?,
    })).map_err(CoreError::from)
}
```

> **设计说明**：「保存版本」时不接受前端塞过来的 snapshot——直接从 DB 读 `canvas_layout` 当前行作为 snapshot 源。这避免：(a) 前端塞错 snapshot 内容；(b) 前端有未保存的临时变更被错误固化。**先保存 layout → 再创建 checkpoint** 是正确顺序，前端调用时要保证这点（详见 §"前端工具栏 onClick"）。

### Tauri command

`src-tauri/src/commands/episode_checkpoint.rs`：

```rust
#[tauri::command]
#[specta::specta]
pub async fn create_episode_checkpoint(
    state: State<'_, AppState>,
    input: CreateCheckpointInput,
) -> Result<EpisodeCheckpoint, IpcError> {
    state.db.call(move |c| {
        crate::db::queries::episode_checkpoint::insert_minimal(c, &input).map_err(Into::into)
    }).await.map_err(IpcError::from)
}
```

注册到 `make_builder()` collect_commands!。

### 前端 zundo middleware

`src/stores/canvasStore.ts` 增强：

```ts
import { temporal } from "zundo";
import { create } from "zustand";
import isEqual from "fast-deep-equal"; // 已在 spec-22 间接引入？若没有则用浅比较——见下面备注
import { shallow } from "zustand/shallow";

const UNDO_STACK_LIMIT = 50;

interface CanvasState {
  nodes: Node[];
  edges: Edge[];
  viewport: Viewport;
  // ...其他 action 同 spec-21
}

export const useCanvasStore = create<CanvasState>()(
  temporal(
    (set, get) => ({
      // spec-21 / spec-22 已有的所有字段和 actions
      nodes: [],
      edges: [],
      viewport: { x: 0, y: 0, zoom: 1 },
      // ...
    }),
    {
      limit: UNDO_STACK_LIMIT,
      // 仅追踪 nodes + edges。viewport 拖动 / 缩放不进 undo 栈
      partialize: (state) => ({
        nodes: state.nodes,
        edges: state.edges,
      }),
      // 等值判断：避免 React Flow 内部把同值 nodes 数组替换触发 snapshot
      equality: (a, b) => shallow(a.nodes, b.nodes) && shallow(a.edges, b.edges),
    },
  ),
);
```

> **设计说明**：
>
> 1. `partialize` 把 `viewport` 排除：用户拖动画布不应该被 Ctrl+Z 撤销
> 2. `equality` 用 `zustand/shallow` 比较 `nodes` 和 `edges` 数组引用 + 浅层元素引用——React Flow 12 内部偶尔会因为 `applyNodeChanges` 返回新数组但元素引用未变，套上 `shallow` 可以避免无效 snapshot
> 3. `limit: 50` 是任务文档要求；zundo 内部 FIFO 丢弃最早状态
> 4. **不用** `fast-deep-equal`——其性能在 50+ 节点画布上有可观开销，shallow 已经够

### 快捷键 hook

`src/components/canvas/toolbar/useCanvasShortcuts.ts`：

```ts
import { useEffect } from "react";
import { useCanvasStore } from "@/stores/canvasStore";

const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

export function useCanvasShortcuts() {
  useEffect(() => {
    const { undo, redo, pastStates } = useCanvasStore.temporal.getState();

    const handler = (e: KeyboardEvent) => {
      // 不拦截输入控件
      const target = e.target as HTMLElement;
      if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable)) {
        return;
      }

      const mod = isMac ? e.metaKey : e.ctrlKey;
      if (!mod) return;

      // Cmd+Z / Ctrl+Z (no shift) → undo
      if (e.key.toLowerCase() === "z" && !e.shiftKey) {
        e.preventDefault();
        useCanvasStore.temporal.getState().undo();
      }
      // Cmd+Shift+Z / Ctrl+Y / Ctrl+Shift+Z → redo
      else if ((e.key.toLowerCase() === "z" && e.shiftKey) || e.key.toLowerCase() === "y") {
        e.preventDefault();
        useCanvasStore.temporal.getState().redo();
      }
    };

    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, []);
}
```

> **设计说明**：用 `document.addEventListener` 而非 React onKeyDown，因为画布上的快捷键应当全局生效（不要求 ReactFlow 元素 focus）；textarea/input 时 return 让原生 undo 接管输入框 undo。

### CanvasToolbar 组件

`src/components/canvas/toolbar/CanvasToolbar.tsx`：

```tsx
import { useReactFlow } from "@xyflow/react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Button } from "@/components/ui/button";
import { ZoomIn, ZoomOut, Maximize, Undo2, Redo2, BookmarkPlus } from "lucide-react";
import { useCanvasStore } from "@/stores/canvasStore";
import { useCreateCheckpoint } from "@/hooks/useCheckpoint";
import { toast } from "sonner";
import { ZoomIndicator } from "./ZoomIndicator";

export function CanvasToolbar({ episodeId }: { episodeId: string }) {
  const { zoomIn, zoomOut, fitView } = useReactFlow();
  const pastStates = useCanvasStore.temporal((s) => s.pastStates);
  const futureStates = useCanvasStore.temporal((s) => s.futureStates);
  const createCheckpoint = useCreateCheckpoint();

  const onSaveVersion = async () => {
    // 1. 触发自动保存的 flush，确保 canvas_layout 已落库
    await flushAutoSave();
    // 2. 后端从 DB 读 canvas_layout 创建 checkpoint
    try {
      await createCheckpoint.mutateAsync({ episode_id: episodeId, note: "" });
      toast.success("已保存版本");
    } catch (e) {
      toast.error(`保存版本失败：${e}`);
    }
  };

  return (
    <div className="absolute left-4 top-4 z-10 flex items-center gap-1 rounded-md border bg-background p-1 shadow">
      <ToolbarBtn icon={ZoomOut} tooltip="缩小 (Cmd+-)" onClick={() => zoomOut({ duration: 200 })} />
      <ZoomIndicator />
      <ToolbarBtn icon={ZoomIn} tooltip="放大 (Cmd++)" onClick={() => zoomIn({ duration: 200 })} />
      <ToolbarBtn icon={Maximize} tooltip="适应视图" onClick={() => fitView({ padding: 0.2, duration: 300 })} />
      <Separator />
      <ToolbarBtn icon={Undo2} tooltip="撤销 (Cmd+Z)" disabled={pastStates.length === 0}
        onClick={() => useCanvasStore.temporal.getState().undo()} />
      <ToolbarBtn icon={Redo2} tooltip="重做 (Cmd+Shift+Z)" disabled={futureStates.length === 0}
        onClick={() => useCanvasStore.temporal.getState().redo()} />
      <Separator />
      <ToolbarBtn icon={BookmarkPlus} tooltip="保存版本" onClick={onSaveVersion} />
    </div>
  );
}

function ToolbarBtn({ icon: Icon, tooltip, ...props }: ToolbarBtnProps) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button size="icon" variant="ghost" className="h-7 w-7" {...props}>
          <Icon className="h-4 w-4" />
        </Button>
      </TooltipTrigger>
      <TooltipContent side="bottom">{tooltip}</TooltipContent>
    </Tooltip>
  );
}
```

### ZoomIndicator

```tsx
// src/components/canvas/toolbar/ZoomIndicator.tsx
import { useStore as useReactFlowInternalStore } from "@xyflow/react";

export function ZoomIndicator() {
  const zoom = useReactFlowInternalStore((s) => s.transform[2]);
  return (
    <span className="px-2 text-xs tabular-nums text-muted-foreground">
      {Math.round(zoom * 100)}%
    </span>
  );
}
```

> **设计说明**：用 React Flow 内部 `useStore` 拿实时 zoom，比 `useReactFlow().getZoom()` 在拖动期间响应快——后者只在事件回调里 fresh。

### useCreateCheckpoint hook

`src/hooks/useCheckpoint.ts`：

```ts
import { useMutation } from "@tanstack/react-query";
import { commands } from "@/lib/bindings";

export function useCreateCheckpoint() {
  return useMutation({
    mutationFn: (input: CreateCheckpointInput) => commands.createEpisodeCheckpoint(input),
  });
}
```

不在 hook 里做 toast——交给 caller 处理（toolbar 已处理）。

### flushAutoSave 衔接

spec-21 的 `useAutoSaveLayout` 返回值需要扩展，暴露 `flush()` 方法供 toolbar 调用：

```ts
// useAutoSave.ts 增强
export function useAutoSaveLayout(episodeId: string) {
  const save = useDebouncedCallback(/* ... */, AUTO_SAVE_DEBOUNCE_MS);
  // ...subscribe 逻辑同 spec-21

  // 暴露 flush 函数给外部
  return { flush: () => save.flush() };
}
```

然后在 CanvasInner 用 ref 把它传给 toolbar 的 `onSaveVersion`：

```tsx
const autoSave = useAutoSaveLayout(episodeId);
// ...
<CanvasToolbar episodeId={episodeId} onBeforeSaveVersion={autoSave.flush} />
```

> **设计说明**：避免 toolbar 直接耦合 `useAutoSaveLayout` —— toolbar 只需要知道「在调 backend 之前要 flush 一下」，具体怎么 flush 由 CanvasInner 编排。

### CanvasInner 装配

```tsx
export function CanvasInner({ episodeId }: { episodeId: string }) {
  // spec-21 已有的 nodes/edges/onChange
  useCanvasShortcuts();
  const autoSave = useAutoSaveLayout(episodeId);

  return (
    <div className="relative h-full" onDragOver={(e) => e.preventDefault()} onDrop={onInternalDrop}>
      <ReactFlow {...} />
      <CanvasToolbar episodeId={episodeId} onBeforeSaveVersion={autoSave.flush} />
      <DropTargetOverlay show={isDragOver} />
      <AssetDrawer projectId={projectId} />
    </div>
  );
}
```

### Undo 与业务副作用的边界

spec-22 定义了「character→storyboard 连线时同步写 `shot_character` 表」的副作用。撤销该连线时是否反向 unlink？**本 spec 决议：不反向**。

| 操作 | 画布 store | 业务表 |
|---|---|---|
| 拖出 character→shot 连线 | edges 新增 | shot_character INSERT |
| Ctrl+Z 撤销 | edges 回滚（前一帧 snapshot） | **不动**（shot_character 行保留） |
| 再次 Ctrl+Y 重做 | edges 新增（重放） | INSERT OR IGNORE（幂等，相同行不重复） |
| 主动按 Delete 删边 | edges 删除 | shot_character DELETE（spec-22 `onNodesChange` 拦截） |

理由：

1. **撤销的是 UI 意图，不是数据真相**：用户 Ctrl+Z 期望撤销刚才的"画布动作"，而不是回滚 DB 写入
2. **业务表副作用可能是用户在画布外触发的**（如未来的"在分镜详情页绑定角色"）—— undo 栈不应该跨越业务边界
3. **如果反向同步**，撤销一个误删可能引发"撤销 → 重新关联角色 → 但角色已被另一处逻辑解绑"的脏数据
4. 代价：用户撤销后画布上没了边，但 `shot_character` 表里还有 row。下次重新打开画布 → spec-22 不自动从关联表恢复 edge（spec-22 已声明此 trade-off）→ 这条孤立 row 直到用户手动重新连线才被"复活"，否则只能在 EpisodeDetailPage 看到该 character 仍在该 shot 的关联列表里。这种"孤立"在生产中影响极小（用户基本不会 undo 复杂连线）

### 错误场景

| 场景 | 行为 |
|---|---|
| `create_episode_checkpoint` 时 canvas_layout 不存在 | core 返回 `NotFound`；IPC 错；toast「请先在画布上操作以初始化 layout」 |
| Undo 栈空时按 Cmd+Z | `useCanvasStore.temporal.getState().undo()` 内部静默忽略；toolbar 按钮 `disabled={pastStates.length === 0}` 视觉禁用 |
| 用户在 textarea 编辑 ScriptNode 内容时按 Cmd+Z | hook 检测到 INPUT/TEXTAREA target，**不**拦截，浏览器接管 textarea 自身的 undo |
| 工具栏 fitView 时画布无任何节点 | React Flow `fitView` 自动 no-op；视觉上无变化，不报错 |
| 快速连按 Cmd+Z 50+ 次 | zundo `pastStates` 用尽后 undo no-op；按钮 disabled |
| Undo 操作触发自动保存 | autoSave 的 subscribe callback 检测到 store 变化 → debounce 1s → 哈希对比 → 写库；正常行为 |
| 保存版本时正好有 in-flight autoSave mutation | `await autoSave.flush()` 触发同步 debounce flush；mutation 串行化由 TanStack Query 保证；checkpoint 读到的 layout 一定是最新的 |

### 测试策略

#### Rust 单元测试

`crates/core/src/db/queries/episode_checkpoint.rs`：
- `insert_minimal` 时 canvas_layout 存在 → 创建成功 + snapshot JSON 含 nodes_json/edges_json/viewport_json 三键
- `insert_minimal` 时 canvas_layout 不存在 → 返回 NotFound
- `note` 默认空字符串

#### 前端单元测试

`src/components/canvas/toolbar/useCanvasShortcuts.test.ts`：
- 按 Cmd+Z（mock metaKey + key=z）→ `useCanvasStore.temporal.getState().undo` 被调
- 按 Cmd+Shift+Z → redo 被调
- 在 input.focus 时按 Cmd+Z → undo **不**被调
- 按 Cmd+A → 不拦截、undo 不被调

`src/components/canvas/toolbar/CanvasToolbar.test.tsx`：
- Undo 按钮在 pastStates 空时 disabled
- 点 Undo 按钮 → undo 被调
- 点「保存版本」→ autoSave.flush 被先调 → createCheckpoint.mutateAsync 被调 → toast 成功

`src/stores/canvasStore.temporal.test.ts`：
- 初始 store → 加节点 → undo → nodes 回到空
- 加节点 → 移动节点（viewport 不变）→ undo → 节点回到 add 前一帧
- viewport 变化不触发 temporal snapshot（pastStates.length 不变）
- 添加 60 节点 → pastStates.length === 50（栈深限制）

#### 手动验证清单

- [ ] 画布上右键创建一个 StoryboardNode → 按 Cmd+Z → 节点消失
- [ ] 按 Cmd+Shift+Z → 节点重现
- [ ] 拖动画布平移 → 按 Cmd+Z → 节点不变（viewport 不在 undo 栈）
- [ ] 在 ScriptNode textarea 里输入文字 → 按 Cmd+Z → textarea 内文字撤销（不是画布撤销）
- [ ] 工具栏放大/缩小按钮 → ZoomIndicator 百分比实时更新
- [ ] 「适应视图」按钮 → 画布自动 zoom 到容纳所有节点
- [ ] 「保存版本」按钮 → toast「已保存版本」→ DB 直查 `SELECT * FROM episode_checkpoint WHERE episode_id = '...'` 看到新行 + snapshot 字段含完整 JSON
- [ ] 连按 Cmd+Z 60 次 → 第 51 次起按钮 disabled
- [ ] Cmd+Z 撤销「拖 character → storyboard 连线」→ 画布上边消失，但 DB `shot_character` 行**仍在**（按本 spec §"Undo 与业务副作用的边界" 的决议）

## 验收标准

- [ ] `canvasStore` 套 `temporal` middleware，`partialize` 仅 `{nodes, edges}`
- [ ] 栈深度 50；`equality` 配置避免无效 snapshot
- [ ] `useCanvasShortcuts` 全局监听 Cmd+Z / Cmd+Shift+Z / Cmd+Y；input/textarea focus 时不拦截
- [ ] `<CanvasToolbar>` 五个按钮 + ZoomIndicator 完整实现
- [ ] `<CanvasToolbar>` 用 shadcn tooltip 给每个按钮加 hover 提示
- [ ] `useAutoSaveLayout` 暴露 `flush()` 方法
- [ ] `episode_checkpoint` 的 model + insert_minimal query + 1 个 IPC 完成
- [ ] 「保存版本」按钮先 flush autoSave、再 createCheckpoint、最后 toast
- [ ] `cargo test -p mango-core` 至少新增 3 个 checkpoint 测试
- [ ] `cargo clippy -p mango-core --all-targets -D warnings` 干净
- [ ] `pnpm test:run` 至少新增 8 个前端测试
- [ ] `pnpm typecheck` 干净
- [ ] `cargo test -p mango-tauri export_bindings` 重新生成 commands.ts diff 仅含 1 个新命令
- [ ] 手动验证清单全过

## 依赖关系

- **前置**：
  - spec-21（`canvasStore` 必须先存在才能套 middleware；`useAutoSaveLayout` 已建才能加 flush 方法）
  - spec-22（业务节点已建，撤销/重做才有实际操作对象）
  - spec-23（建议拖放先合，避免 zundo 套上后拖放产生的 addNode 是否进栈的来回 review）
- **平行**：无
- **后续**：
  - MS4 将本 spec 创建的 `episode_checkpoint` 行扩展为完整版本管理（list / restore / 自动清理 / SQLite backup）

## 与原任务文档的差异

合并自原任务 11（Undo/Redo）+ 任务 12（画布工具栏）。

- **原任务 11 没明确"撤销是否反向写关联表"**：本 spec 决议「不反向」并写明理由与代价
- **原任务 11 没说 viewport 是否进 undo 栈**：本 spec 决议「不进」（`partialize` 排除）—— 拖画布不应该被 Ctrl+Z 撤销，这是常见 IDE 约定
- **原任务 12 的"保存版本"含义模糊**：本 spec 明确 MS3 范围内是「占位」—— DB 写一行，但 list/restore 等完整功能属 MS4
- **原任务 12 没提工具栏与 React Flow zoom 状态同步**：本 spec 用 `useStore` hook 实时读 `transform[2]`，避免 stale zoom
- **原任务 12 没提 flush autoSave 与 checkpoint 的顺序**：本 spec 明确"先 flush → 再 createCheckpoint"，否则 checkpoint 读到的可能是上一个 debounce 周期的 layout
