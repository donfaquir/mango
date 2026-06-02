# SPEC-24: Undo/Redo + 画布工具栏 + EpisodeCheckpoint 占位

## 概述

给 spec-21 建立的 `canvasStore` 套上 `zundo` temporal middleware，实现 Ctrl+Z / Ctrl+Y / Cmd+Shift+Z 撤销重做；在画布顶部叠加 `<CanvasToolbar>`，提供缩放 / 适应视图 / 撤销 / 重做 / 保存版本五个工具按钮。

「保存版本」按钮在本 spec 中**仅写入一条 `episode_checkpoint` 行**作为占位——完整的版本管理（恢复 / 列表 / 自动清理 / SQLite backup）属于 MS4 范畴。本 spec 保证按钮和 IPC 命令可点击、不报错、DB 有写入痕迹，让 MS4 可以无缝接续。

> ⚠️ **破坏性变更**：`canvasStore` 引入 `temporal` middleware，store 形状不变但调用方式新增 `useCanvasStore.temporal.getState()`；`canvas_layout` 之外新增 `episode_checkpoint` 表的 1 个 Tauri command（`create_episode_checkpoint`，占位实现）。

## 设计目标 / 非目标

**目标**：

- `canvasStore` 套 `zundo` middleware，`partialize` 仅追踪 `{ nodes, edges }`，**不追踪** viewport（避免拖动画布污染 undo 栈）
- 栈深度 50
- **节点拖动期不污染栈**：拖动过程中 `pause()`，`onNodeDragStop` 时 `resume()` —— 一次拖动 = 一帧 snapshot
- **`init()` / `reset()` 不污染栈**：加载已存 layout 不应该被 Cmd+Z 一键清空 —— 用 `pause()/resume()` 包裹，然后 `clear()` 旧栈
- 快捷键：Ctrl+Z / Cmd+Z 撤销；Cmd+Shift+Z（macOS）/ Ctrl+Y / Ctrl+Shift+Z（非 macOS）重做。**macOS 上不接受 Cmd+Y**（系统约定 Cmd+Y 是其他功能）。textarea/input/contentEditable focus 时**不**拦截
- `<CanvasToolbar>` 五个按钮 + 缩放百分比指示器，置于画布左上角；shadcn `tooltip` 给每个按钮 hover 提示
- 「保存版本」按钮 → 先 `flush()` 自动保存确保 `canvas_layout` 落库 → 再调 `create_episode_checkpoint(input)` 抓 `canvas_layout` 写入 `episode_checkpoint` → toast 「已保存版本」
- 「适应视图」用 React Flow 12 `useReactFlow().fitView({ padding: 0.2, duration: 300 })`
- `episode_checkpoint` Rust model + 1 个 query `insert_minimal` + 1 个 IPC

**非目标**（明确划归后续 spec / MS）：

- EpisodeCheckpoint 列表 UI / 恢复操作 / 自动清理策略 → MS4
- SQLite backup（`VACUUM INTO`）→ MS4
- Undo/Redo 跨业务副作用（如撤销 character→shot 连线时反向重建 `shot_character` 行）→ **本 spec 决议「只撤销画布层 nodes/edges，业务表副作用一次性写入，撤销不回放」**；详见 §"Undo 与业务副作用的边界"
- 工具栏更多按钮（导出 / 截图 / 锁定视图）→ V2
- 触屏 / 手势缩放 → V2
- `script_text` / `shots_json` / `change_summary` / `version_number` 升档（如增量保存策略、change_summary 自动归纳）→ MS4

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
│   ├── CanvasInner.tsx                 # spec-21 已建，本 spec 加 <CanvasToolbar /> 渲染 + 接 onNodeDragStart/Stop
│   ├── toolbar/
│   │   ├── CanvasToolbar.tsx           # 新增：五按钮 + 缩放指示器
│   │   ├── ZoomIndicator.tsx           # 新增
│   │   └── useCanvasShortcuts.ts       # 新增：全局键盘 hook（Cmd+Z / Cmd+Shift+Z / Ctrl+Y）
│   └── persistence/
│       └── useAutoSave.ts              # spec-21 已建，本 spec 改为 return { flush } 暴露 flush
└── stores/
    └── canvasStore.ts                  # spec-21 已建，本 spec 套 temporal middleware + partialize + init/reset pause
```

### 依赖项变更

**npm 新增**：无（`zundo ^2.3.0` 已在 `package.json` 中，已验证）

**shadcn/ui 新增**：`tooltip` 和 `separator`（实际项目中两者都未 add，需要补）

```bash
pnpm dlx shadcn@latest add tooltip
pnpm dlx shadcn@latest add separator
```

**Rust**：无新增。
**Tauri 插件**：无新增。
**Migration**：无（`episode_checkpoint` 表已在 `001_initial.sql:185`）

### `episode_checkpoint` 表实际 schema（来自 MS0 `001_initial.sql:185-198`）

```sql
CREATE TABLE episode_checkpoint (
    id                    TEXT PRIMARY KEY,
    episode_id            TEXT NOT NULL REFERENCES episode(id) ON DELETE CASCADE,
    version_number        INTEGER NOT NULL,                                 -- NOT NULL 无默认值，本 spec 自增
    label                 TEXT,                                             -- 用户备注；本 spec 写入 input.label
    trigger_type          TEXT NOT NULL DEFAULT 'manual'
                                  CHECK(trigger_type IN ('auto','manual')), -- 本 spec 固定 'manual'
    script_text           TEXT NOT NULL DEFAULT '',                         -- MS4 才用，本 spec 用默认值
    shots_json            TEXT NOT NULL DEFAULT '[]',                       -- MS4 才用，本 spec 用默认值
    canvas_nodes_json     TEXT NOT NULL DEFAULT '[]',                       -- 本 spec 写入：canvas_layout.nodes_json
    canvas_edges_json     TEXT NOT NULL DEFAULT '[]',                       -- 本 spec 写入：canvas_layout.edges_json
    canvas_viewport_json  TEXT NOT NULL DEFAULT '{}',                       -- 本 spec 写入：canvas_layout.viewport_json
    change_summary        TEXT,                                             -- MS4 才用
    created_at            TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_checkpoint_episode ON episode_checkpoint(episode_id);
```

> **schema 对齐决议（方案 A，不改 migration）**：
>
> 1. **不引入"snapshot 单列"**：直接复用已存的 3 个 canvas_*_json 列，避免与 MS4 还要再迁移
> 2. **`version_number` 自增**：`SELECT COALESCE(MAX(version_number), 0) + 1 FROM episode_checkpoint WHERE episode_id = ?1`。SQLite 单写者 + Tauri IPC 串行化保证并发安全
> 3. **`note` 概念对齐 `label`**：spec 早期稿用 `note` 字段名，统一改为 `label`（与 schema 一致）
> 4. **`trigger_type` 固定 `'manual'`**：本 spec 只有手动触发；自动触发由 MS4 增加
> 5. **`script_text / shots_json / change_summary` 留空**：靠 schema 默认值，本 spec 不写入

### Rust 数据模型

`crates/core/src/models/episode_checkpoint.rs`：

```rust
use serde::{Deserialize, Serialize};
use specta::Type;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct EpisodeCheckpoint {
    pub id: String,
    pub episode_id: String,
    pub version_number: i64,
    pub label: Option<String>,
    pub trigger_type: String,
    pub canvas_nodes_json: String,
    pub canvas_edges_json: String,
    pub canvas_viewport_json: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Type, TS)]
#[ts(export, export_to = "../../src/lib/bindings/")]
pub struct CreateCheckpointInput {
    pub episode_id: String,
    /// 用户备注，可空
    #[ts(optional)]
    pub label: Option<String>,
}
```

> 只在 Rust 模型里暴露本 spec 需要的列；`script_text / shots_json / change_summary` MS4 再加。`#[ts(optional)]` 让 TS 类型变 `label?: string | null`，前端可不传字段。

### Rust 查询层

`crates/core/src/db/queries/episode_checkpoint.rs`：

```rust
use rusqlite::{params, Connection};
use crate::{Result, CoreError};
use crate::models::episode_checkpoint::*;
use crate::db::queries::canvas_layout as canvas_q;

/// 最小化创建：抓取当前 episode 的 canvas_layout 作为 snapshot 写入。
/// 不实现 list / restore / delete —— 那些由 MS4 实施。
pub fn insert_minimal(
    conn: &Connection,
    input: &CreateCheckpointInput,
) -> Result<EpisodeCheckpoint> {
    let layout = canvas_q::get_by_episode(conn, &input.episode_id)?
        .ok_or_else(|| CoreError::NotFound {
            entity: "canvas_layout",
            id: input.episode_id.clone(),
        })?;

    let next_version: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version_number), 0) + 1
         FROM episode_checkpoint WHERE episode_id = ?1",
        params![input.episode_id],
        |r| r.get(0),
    )?;

    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO episode_checkpoint
            (id, episode_id, version_number, label, trigger_type,
             canvas_nodes_json, canvas_edges_json, canvas_viewport_json)
         VALUES (?1, ?2, ?3, ?4, 'manual', ?5, ?6, ?7)",
        params![
            id,
            input.episode_id,
            next_version,
            input.label,
            layout.nodes_json,
            layout.edges_json,
            layout.viewport_json,
        ],
    )?;

    let mut stmt = conn.prepare(
        "SELECT id, episode_id, version_number, label, trigger_type,
                canvas_nodes_json, canvas_edges_json, canvas_viewport_json, created_at
         FROM episode_checkpoint WHERE id = ?1",
    )?;
    stmt.query_row(params![id], |r| {
        Ok(EpisodeCheckpoint {
            id: r.get(0)?,
            episode_id: r.get(1)?,
            version_number: r.get(2)?,
            label: r.get(3)?,
            trigger_type: r.get(4)?,
            canvas_nodes_json: r.get(5)?,
            canvas_edges_json: r.get(6)?,
            canvas_viewport_json: r.get(7)?,
            created_at: r.get(8)?,
        })
    })
    .map_err(CoreError::from)
}
```

> **设计说明**：
>
> 1. 「保存版本」时不接受前端塞过来的 snapshot——直接从 DB 读 `canvas_layout` 当前行作为源。避免 (a) 前端塞错；(b) 前端有未保存变更被错误固化。**先 flush autoSave → 再创建 checkpoint** 是必须顺序，前端调用要保证（详见 §"前端工具栏 onClick"）
> 2. `version_number` 自增的并发安全：SQLite 单写者 + Tauri command 串行化（`tokio_rusqlite::Connection::call` 排队执行）；本应用不存在多进程写同库
> 3. `trigger_type` 固定 `'manual'`：本 spec 只有手动触发，MS4 加自动触发时再扩展

### Tauri command

`src-tauri/src/commands/episode_checkpoint.rs`：

```rust
use tauri::State;
use mango_core::models::episode_checkpoint::{CreateCheckpointInput, EpisodeCheckpoint};
use mango_core::db::queries::episode_checkpoint as q;
use crate::{AppState, IpcError};

#[tauri::command]
#[specta::specta]
pub async fn create_episode_checkpoint(
    state: State<'_, AppState>,
    input: CreateCheckpointInput,
) -> Result<EpisodeCheckpoint, IpcError> {
    state
        .db
        .call(move |c| {
            q::insert_minimal(c, &input)
                .map_err(|e| tokio_rusqlite::Error::Other(Box::new(e)))
        })
        .await
        .map_err(IpcError::from)
}
```

> **闭包签名说明**：`tokio_rusqlite::Connection::call` 的闭包参数是 `&mut Connection`，但 `insert_minimal` 接受 `&Connection`，依靠 `&mut T` → `&T` 的隐式 coerce 通过。错误链：`CoreError` 包成 `tokio_rusqlite::Error::Other`，最后 `IpcError::from(tokio_rusqlite::Error)` 转字符串。**实施前 grep 一下 `src-tauri/src/commands/asset.rs` 等已有 command 的错误转换写法，对齐风格**。

注册到 `make_builder()` 的 `collect_commands!` 列表。

### 前端 zundo middleware

`src/stores/canvasStore.ts` 增强：

```ts
import { temporal } from "zundo";
import { create } from "zustand";
import type { TemporalState } from "zundo";
// ...spec-21 / spec-22 已有的 import

const UNDO_STACK_LIMIT = 50;

type PartialCanvasState = Pick<CanvasState, "nodes" | "edges">;

export const useCanvasStore = create<CanvasState>()(
  temporal(
    (set, get) => ({
      // spec-21 / spec-22 已有的所有字段和 actions
      nodes: [],
      edges: [],
      viewport: EMPTY_VIEWPORT,
      // ...其他 actions 保持不变

      // ❗ init 必须包 pause/resume + clear，否则首次加载就会污染栈
      init: (s) => {
        const temporal = useCanvasStore.temporal.getState();
        temporal.pause();
        set({ nodes: s.nodes, edges: s.edges, viewport: s.viewport });
        temporal.resume();
        temporal.clear();
      },

      // ❗ reset 同理
      reset: () => {
        const temporal = useCanvasStore.temporal.getState();
        temporal.pause();
        set({ nodes: [], edges: [], viewport: EMPTY_VIEWPORT });
        temporal.resume();
        temporal.clear();
      },

      // ...其他 actions 不变（addNode / updateNodeData / onConnect / onNodesChange / onEdgesChange / setViewport）
    }),
    {
      limit: UNDO_STACK_LIMIT,
      // 仅追踪 nodes + edges。viewport 拖动 / 缩放不进 undo 栈
      partialize: (state): PartialCanvasState => ({
        nodes: state.nodes,
        edges: state.edges,
      }),
    },
  ),
);

// 暴露 temporal store 类型给消费方
export type CanvasTemporalState = TemporalState<PartialCanvasState>;
```

> **设计说明**：
>
> 1. `partialize` 排除 `viewport` —— 拖画布不应该被 Ctrl+Z 撤销，符合常见 IDE 约定
> 2. `partialize` 排除所有 action 函数 —— zundo 内部 `set(partial)` 走 merge 模式，actions 不会被覆盖。**这点要在测试里 assert 一次**（undo 后 `useCanvasStore.getState().addNode` 仍是函数）
> 3. `init/reset` 用 `pause()/resume() + clear()` 包裹：否则首次加载已存 layout 会向 `pastStates` 推一帧，用户按一下 Cmd+Z 就把整个画布清空
> 4. **不配置 `equality`**：`zustand/shallow` 防不住 React Flow `applyNodeChanges` 在 drag 期返回的新 position 对象。`fast-deep-equal` 性能在 50+ 节点画布上有可观开销。本 spec 用「drag 期 pause」方案替代等值判断（见下节）
> 5. `limit: 50` 是任务文档要求；zundo 内部 FIFO 丢弃最早状态

### 节点拖动期防抖（关键决议）

**问题**：React Flow 12 在节点拖动期间，每个 mousemove 都会触发 `onNodesChange({ type: 'position', dragging: true, ... })`，`applyNodeChanges` 返回**新元素对象**（新 position）。如果不处理，一次拖动会产生几十帧 snapshot，用户按一次 Cmd+Z 只能撤一个像素。

**方案选型**：

| 方案 | 复杂度 | 优劣 |
|---|---|---|
| zundo `equality` + shallow | 低 | ❌ 救不了，shallow 对新对象引用直接判不等 |
| zundo `handleSet` + throttle 200ms | 中 | ⚠️ 通用，但难调；节流期内有别的 set 也会被合并 |
| **React Flow `onNodeDragStart/Stop` 手动 pause/resume** | 低 | ✅ 语义清晰；仅作用于拖动；其它操作（addNode/onConnect/remove）保持默认入栈 |

**采用方案**：手动 pause/resume。`CanvasInner` 上挂：

```tsx
const onNodeDragStart = useCallback(() => {
  useCanvasStore.temporal.getState().pause();
}, []);

const onNodeDragStop = useCallback(() => {
  const temporal = useCanvasStore.temporal.getState();
  temporal.resume();
  // 拖动期产生的所有中间帧已被忽略；resume 后下一次 set 才会入栈。
  // 这里手动 push 一帧"拖动终态"快照，让 Cmd+Z 能撤这次拖动：
  // 调用 zundo 内部 _handleSet 不优雅；改为让 React Flow 在 resume 后再
  // dispatch 一次 onNodesChange（无 op）触发 set。实际上 dragStop 之后
  // React Flow 会自动 dispatch 一次 dragging:false 的最终 position change，
  // 那一次会被 zundo 捕获 —— 验证手段：测试用例《拖动后 Ctrl+Z 回到拖动前位置》
}, []);

return (
  <ReactFlow
    onNodeDragStart={onNodeDragStart}
    onNodeDragStop={onNodeDragStop}
    // ...
  />
);
```

> ⚠️ **实施验证项**：在写完后必须验证「拖动一个节点 100 像素 → onNodeDragStop 后栈中只新增 1 帧（不是 100 帧）→ Cmd+Z 后节点回到拖动前位置」。如果 React Flow 没有在 dragStop 后再 dispatch 一次终态 change，则需要在 `onNodeDragStop` 里手动 `setNodes(...nodes)` 触发一次。

### 快捷键 hook

`src/components/canvas/toolbar/useCanvasShortcuts.ts`：

```ts
import { useEffect } from "react";
import { useCanvasStore } from "@/stores/canvasStore";

const isMac =
  typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

export function useCanvasShortcuts() {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // 不拦截输入控件
      const target = e.target as HTMLElement | null;
      if (
        target &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.isContentEditable)
      ) {
        return;
      }

      const mod = isMac ? e.metaKey : e.ctrlKey;
      if (!mod) return;

      const key = e.key.toLowerCase();

      // 撤销：Cmd+Z / Ctrl+Z（无 shift）
      if (key === "z" && !e.shiftKey) {
        e.preventDefault();
        useCanvasStore.temporal.getState().undo();
        return;
      }

      // 重做：
      //  - macOS: Cmd+Shift+Z（不接受 Cmd+Y —— 系统约定）
      //  - 非 mac: Ctrl+Shift+Z 或 Ctrl+Y
      const isRedo =
        (key === "z" && e.shiftKey) || (!isMac && key === "y");
      if (isRedo) {
        e.preventDefault();
        useCanvasStore.temporal.getState().redo();
      }
    };

    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, []);
}
```

> **设计说明**：
>
> 1. 用 `document.addEventListener` 而非 React onKeyDown —— 画布快捷键应当全局生效（不要求 ReactFlow 元素 focus）
> 2. textarea/input/contentEditable focus 时 return，让原生 undo 接管输入框 undo
> 3. **macOS 不接受 Cmd+Y**：macOS 系统约定 Cmd+Y 是其他功能（Finder 是"显示历史"、Safari 是"显示历史记录"），用户预期 Cmd+Y 不是 redo
> 4. 不在 useEffect 顶部预解构 `undo/redo/pastStates` —— handler 内部用最新 `getState()` 取，避免闭包捕获过期函数引用

### CanvasToolbar 组件

`src/components/canvas/toolbar/CanvasToolbar.tsx`：

```tsx
import { useReactFlow } from "@xyflow/react";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { Separator } from "@/components/ui/separator";
import { Button } from "@/components/ui/button";
import {
  BookmarkPlus,
  Maximize,
  Redo2,
  Undo2,
  ZoomIn,
  ZoomOut,
} from "lucide-react";
import type { ComponentProps, ElementType } from "react";
import { useStore as useReactFlowInternalStore } from "@xyflow/react";
import { useCanvasStore } from "@/stores/canvasStore";
import { useCreateCheckpoint } from "@/hooks/useCheckpoint";
import { toast } from "sonner";
import { ZoomIndicator } from "./ZoomIndicator";

interface CanvasToolbarProps {
  episodeId: string;
  /**
   * Caller-provided hook to flush any pending autoSave debounce *before*
   * the checkpoint is created — ensures the snapshot read from DB is current.
   */
  onBeforeSaveVersion?: () => Promise<void> | void;
}

export function CanvasToolbar({
  episodeId,
  onBeforeSaveVersion,
}: CanvasToolbarProps) {
  const { zoomIn, zoomOut, fitView } = useReactFlow();
  const pastStatesLen = useReactFlowInternalStoreTemporalLen("past");
  const futureStatesLen = useReactFlowInternalStoreTemporalLen("future");
  const createCheckpoint = useCreateCheckpoint();

  const onSaveVersion = async () => {
    try {
      await onBeforeSaveVersion?.();
      await createCheckpoint.mutateAsync({ episode_id: episodeId });
      toast.success("已保存版本");
    } catch (e) {
      toast.error(`保存版本失败：${String(e)}`);
    }
  };

  return (
    <div className="absolute left-4 top-4 z-10 flex items-center gap-1 rounded-md border bg-background p-1 shadow">
      <ToolbarBtn
        icon={ZoomOut}
        tooltip="缩小"
        onClick={() => zoomOut({ duration: 200 })}
      />
      <ZoomIndicator />
      <ToolbarBtn
        icon={ZoomIn}
        tooltip="放大"
        onClick={() => zoomIn({ duration: 200 })}
      />
      <ToolbarBtn
        icon={Maximize}
        tooltip="适应视图"
        onClick={() => fitView({ padding: 0.2, duration: 300 })}
      />
      <Separator orientation="vertical" className="mx-1 h-5" />
      <ToolbarBtn
        icon={Undo2}
        tooltip="撤销 (Cmd+Z)"
        disabled={pastStatesLen === 0}
        onClick={() => useCanvasStore.temporal.getState().undo()}
      />
      <ToolbarBtn
        icon={Redo2}
        tooltip="重做 (Cmd+Shift+Z)"
        disabled={futureStatesLen === 0}
        onClick={() => useCanvasStore.temporal.getState().redo()}
      />
      <Separator orientation="vertical" className="mx-1 h-5" />
      <ToolbarBtn
        icon={BookmarkPlus}
        tooltip="保存版本"
        onClick={onSaveVersion}
        disabled={createCheckpoint.isPending}
      />
    </div>
  );
}

interface ToolbarBtnProps
  extends Omit<ComponentProps<typeof Button>, "children"> {
  icon: ElementType;
  tooltip: string;
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

// 用 zundo 的 useStore 形式订阅 past/future 长度变化，按钮 disable 才会响应式
function useReactFlowInternalStoreTemporalLen(which: "past" | "future") {
  return useCanvasStore.temporal(
    (s) => (which === "past" ? s.pastStates.length : s.futureStates.length),
  );
}
```

> **设计说明**：
>
> 1. `pastStatesLen / futureStatesLen` 通过 zundo 提供的 `useCanvasStore.temporal(selector)` 订阅 —— 否则按钮 disabled 只在首次渲染评估，undo/redo 后不更新
> 2. tooltip 文案省去具体快捷键平台差异（Cmd/Ctrl）—— 移到平台无关的简短描述；如想精细化可用 `isMac` 切换
> 3. `disabled={createCheckpoint.isPending}` 防止用户连点保存版本
> 4. `<Separator orientation="vertical" className="mx-1 h-5" />` —— shadcn `separator` 默认水平，必须显式传 `orientation="vertical"` 并给高度

### ZoomIndicator

```tsx
// src/components/canvas/toolbar/ZoomIndicator.tsx
import { useStore as useReactFlowInternalStore } from "@xyflow/react";

export function ZoomIndicator() {
  const zoom = useReactFlowInternalStore((s) => s.transform[2]);
  return (
    <span className="inline-block w-12 text-center text-xs tabular-nums text-muted-foreground">
      {Math.round(zoom * 100)}%
    </span>
  );
}
```

> **设计说明**：
>
> 1. 用 React Flow 内部 `useStore` 拿实时 zoom，比 `useReactFlow().getZoom()` 在拖动期间响应快
> 2. `inline-block w-12 text-center` 防止 `10%` 和 `100%` 字符数不同导致按钮组左右抖动；`tabular-nums` 让数字等宽

### useCreateCheckpoint hook

`src/hooks/useCheckpoint.ts`：

```ts
import { useMutation } from "@tanstack/react-query";
import { commands } from "@/lib/bindings/commands";
import type { CreateCheckpointInput } from "@/lib/bindings/CreateCheckpointInput";

export function useCreateCheckpoint() {
  return useMutation({
    mutationFn: (input: CreateCheckpointInput) =>
      commands.createEpisodeCheckpoint(input),
  });
}
```

不在 hook 里做 toast —— 交给 caller 处理（toolbar 已处理）。
本 spec 不做 list/restore，所以不需要 `invalidateQueries` —— MS4 接入列表 UI 时再加。

### flushAutoSave 衔接

spec-21 的 `useAutoSaveLayout` 当前签名为 `(episodeId: string) => void`，本 spec 改造为返回 `{ flush }`：

```ts
// src/components/canvas/persistence/useAutoSave.ts 增强
export function useAutoSaveLayout(episodeId: string): { flush: () => void } {
  // ...spec-21 已有的全部逻辑保持不变（lastHashRef / mutation / save / subscribe / seed hash）

  return {
    flush: () => save.flush(),
  };
}
```

> **设计说明**：
>
> 1. spec-21 已经在 unmount cleanup 里调过 `save.flush()`，逻辑成熟；本 spec 只新增对外暴露
> 2. `save.flush()` 是 `use-debounce` 提供的同步触发；如有 pending 的 debounce 会立刻执行 callback；无 pending 则 no-op
> 3. **不返回 `mutation` 或 `isPending`** —— toolbar 只关心 "保存版本之前先 flush" 的时序，不关心 autoSave 的内部状态

### CanvasInner 装配

```tsx
export function CanvasInner({ episodeId, initialViewport }: Props) {
  // spec-21/22/23 已有的 store 订阅、useReactFlow、drag/drop 等不变

  useCanvasShortcuts();
  const autoSave = useAutoSaveLayout(episodeId);

  const onNodeDragStart = useCallback(() => {
    useCanvasStore.temporal.getState().pause();
  }, []);

  const onNodeDragStop = useCallback(() => {
    useCanvasStore.temporal.getState().resume();
  }, []);

  return (
    <div
      className="relative h-full w-full"
      onDragOver={internalDrop.onDragOver}
      onDrop={internalDrop.onDrop}
      onDragLeave={internalDrop.onDragLeave}
    >
      <ReactFlow
        {/* spec-21/22/23 已有的所有 props */}
        onNodeDragStart={onNodeDragStart}
        onNodeDragStop={onNodeDragStop}
      >
        {/* Background / MiniMap / Controls 同 spec-21 */}
      </ReactFlow>
      <CanvasToolbar
        episodeId={episodeId}
        onBeforeSaveVersion={autoSave.flush}
      />
      {/* spec-23 已有的 DropTargetOverlay / AssetDrawer / BindAssetToShotDialog */}
      {/* spec-21 已有的 PaneContextMenu / EdgeContextMenu */}
    </div>
  );
}
```

### Undo 与业务副作用的边界

spec-22 定义了「character→storyboard 连线时同步写 `shot_character` 表」「storyboard 节点删除时反向 unlink」等副作用。撤销时是否反向回放？**本 spec 决议：不反向**。

| 操作 | 画布 store | 业务表 |
|---|---|---|
| 拖出 character→shot 连线 | edges 新增 | shot_character INSERT |
| Ctrl+Z 撤销 | edges 回滚（前一帧 snapshot） | **不动**（shot_character 行保留） |
| 再次 Ctrl+Y 重做 | edges 新增（重放） | INSERT OR IGNORE（幂等，相同行不重复） |
| 主动按 Delete 删边 | edges 删除 | shot_character DELETE（spec-22 `onEdgesChange` 拦截） |
| 从素材库 / 系统文件拖入 → addNode | nodes 新增 | （素材已 INSERT 但与画布无强绑定） |
| Ctrl+Z 撤销拖入 | nodes 回滚 | **不动**（asset 行仍在素材库；只是不显示在画布） |
| 内部拖到 shot 节点上 → assign_asset_to_shot | （画布无变化） | shot_id 已更新 |
| Ctrl+Z 撤销 | **不动**（画布没变就没栈帧） | **不动**（赋值不在 undo 范畴） |

理由：

1. **撤销的是 UI 意图，不是数据真相**：用户 Ctrl+Z 期望撤销刚才的"画布动作"，而不是回滚 DB 写入
2. **业务表副作用可能是用户在画布外触发的**（如未来的"在分镜详情页绑定角色"）—— undo 栈不应该跨越业务边界
3. **如果反向同步**，撤销一个误删可能引发"撤销 → 重新关联角色 → 但角色已被另一处逻辑解绑"的脏数据
4. 代价：用户撤销后画布上没了边，但 `shot_character` 表里还有 row。下次重新打开画布 → spec-22 不自动从关联表恢复 edge → 这条孤立 row 直到用户手动重新连线才被"复活"，或在 EpisodeDetailPage 看到该 character 仍在该 shot 的关联列表里。生产中影响极小

### 错误场景

| 场景 | 行为 |
|---|---|
| `create_episode_checkpoint` 时 canvas_layout 不存在 | core 返回 `NotFound`；IPC 错；toast「请先在画布上操作以初始化 layout」 |
| Undo 栈空时按 Cmd+Z | `temporal.undo()` 内部静默忽略；toolbar 按钮 `disabled={pastStatesLen === 0}` 视觉禁用 |
| 用户在 textarea 编辑 ScriptNode 内容时按 Cmd+Z | hook 检测到 INPUT/TEXTAREA/contentEditable target，**不**拦截，浏览器接管 textarea 自身的 undo |
| 工具栏 fitView 时画布无任何节点 | React Flow `fitView` 自动 no-op；视觉上无变化，不报错 |
| 快速连按 Cmd+Z 50+ 次 | zundo `pastStates` 用尽后 undo no-op；按钮 disabled |
| Undo 操作触发自动保存 | autoSave 的 subscribe callback 检测到 store 变化 → debounce 1s → 哈希对比 → 写库；正常行为 |
| 保存版本时正好有 in-flight autoSave mutation | `await onBeforeSaveVersion()` 触发同步 debounce flush；mutation 串行化由 TanStack Query 保证；checkpoint 读到的 layout 一定是最新的 |
| 拖动节点 100 像素松手 | 拖动期 zundo pause；dragStop 后 React Flow dispatch 终态 change → 入栈一帧；Cmd+Z 回到拖动前位置 |
| macOS 上按 Cmd+Y | hook 不识别此组合，浏览器走默认行为（无操作或系统功能） |

### 测试策略

#### Rust 单元测试

`crates/core/src/db/queries/episode_checkpoint.rs`（≥4 个）：

- `insert_minimal_writes_canvas_columns_from_layout`：插入前先 upsert canvas_layout，调用后 SELECT 行包含正确的 nodes/edges/viewport json
- `insert_minimal_increments_version_per_episode`：连续两次 insert，`version_number` 1 → 2；不同 episode_id 各自从 1 起
- `insert_minimal_returns_not_found_without_layout`：未创建 canvas_layout 直接 insert → `CoreError::NotFound`
- `insert_minimal_label_optional`：input.label = None → 写入 NULL；query 回来 label = None
- `insert_minimal_trigger_type_is_manual`：插入后 trigger_type 列恒为 'manual'

#### 前端单元测试

`src/components/canvas/toolbar/useCanvasShortcuts.test.ts`（≥5 个）：

- 按 Cmd+Z（mock metaKey + key=z）→ `useCanvasStore.temporal.getState().undo` 被调
- 按 Cmd+Shift+Z → redo 被调
- macOS 上按 Cmd+Y → **redo 不被调**（platform mock 为 mac）
- 非 macOS 上按 Ctrl+Y → redo 被调
- 在 input.focus 时按 Cmd+Z → undo **不**被调
- 在 contentEditable div focus 时按 Cmd+Z → undo **不**被调

`src/components/canvas/toolbar/CanvasToolbar.test.tsx`（≥4 个）：

- Undo 按钮在 pastStates 空时 disabled
- 点 Undo 按钮 → undo 被调
- 点「保存版本」→ `onBeforeSaveVersion` 被先 await → `createCheckpoint.mutateAsync` 被调 → toast 成功
- 保存版本失败 → toast 错（不抛、不崩溃）
- `createCheckpoint.isPending` 时保存按钮 disabled

`src/stores/canvasStore.temporal.test.ts`（≥5 个）：

- 初始 store → addNode → undo → nodes 回到空
- addNode → setViewport(zoom=2) → undo → nodes 回到空但 viewport 仍是 zoom=2（**viewport 不在 undo 栈**）
- `init({nodes: [...3 nodes], ...})` → pastStates.length === 0（**init 不污染栈**）
- `reset()` → pastStates.length === 0
- 加 60 节点（一个一个 addNode） → pastStates.length === 50（栈深限制）
- undo 后 `useCanvasStore.getState().addNode` 仍是函数（**actions 不被 partialize 覆盖**）

`src/hooks/useCheckpoint.test.ts`（≥1 个）：

- 调用 `useCreateCheckpoint().mutateAsync({episode_id})` → `commands.createEpisodeCheckpoint` 被调

#### 手动验证清单

- [ ] 画布上右键创建一个 StoryboardNode → 按 Cmd+Z → 节点消失
- [ ] 按 Cmd+Shift+Z → 节点重现
- [ ] **首次打开有已存 layout 的 episode → 按 Cmd+Z → 画布不变**（验证 init pause/resume）
- [ ] 拖动一个节点 100 像素 → 松手 → 按一次 Cmd+Z → 节点回到拖动**前**位置（不是回退一个像素）
- [ ] 拖动画布平移 → 按 Cmd+Z → 节点不变（viewport 不在 undo 栈）
- [ ] 在 ScriptNode textarea 里输入文字 → 按 Cmd+Z → textarea 内文字撤销（不是画布撤销）
- [ ] 工具栏放大/缩小按钮 → ZoomIndicator 百分比实时更新；从 10% 切到 100% 不抖动
- [ ] 「适应视图」按钮 → 画布自动 zoom 到容纳所有节点
- [ ] 「保存版本」按钮 → toast「已保存版本」→ DB 直查 `SELECT id, episode_id, version_number, label, trigger_type, canvas_nodes_json FROM episode_checkpoint WHERE episode_id = '...'` 看到新行，trigger_type='manual'，canvas_nodes_json 含完整 JSON
- [ ] 连续两次「保存版本」 → version_number 1 → 2
- [ ] 连按 Cmd+Z 60 次 → 第 51 次起按钮 disabled
- [ ] Cmd+Z 撤销「拖 character → storyboard 连线」→ 画布上边消失，但 DB `shot_character` 行**仍在**（按本 spec §"Undo 与业务副作用的边界" 的决议）
- [ ] macOS 上按 Cmd+Y → 无 redo 发生（系统行为接管，不报错）

## 验收标准

- [ ] `canvasStore` 套 `temporal` middleware，`partialize` 仅 `{nodes, edges}`
- [ ] `canvasStore.init` 和 `canvasStore.reset` 用 `pause/resume + clear` 包裹
- [ ] 栈深度 50；**不**配 `equality`（用 drag pause/resume 替代）
- [ ] `CanvasInner` 接 `onNodeDragStart/onNodeDragStop` 实现拖动期 pause
- [ ] `useCanvasShortcuts` 全局监听 Cmd+Z / Cmd+Shift+Z / Ctrl+Y；input/textarea/contentEditable focus 时不拦截；macOS 不接受 Cmd+Y
- [ ] `<CanvasToolbar>` 五个按钮 + ZoomIndicator 完整实现；undo/redo 按钮订阅 `pastStates/futureStates` 长度变化响应式 disable
- [ ] `<CanvasToolbar>` 用 shadcn tooltip 给每个按钮加 hover 提示
- [ ] `useAutoSaveLayout` 返回 `{ flush: () => void }`
- [ ] `episode_checkpoint` 的 model + `insert_minimal` query + 1 个 IPC 完成；schema 字段名严格对齐 `001_initial.sql:185-198`
- [ ] `version_number` 自增、`trigger_type` 固定 `'manual'`、`label` 可空
- [ ] 「保存版本」按钮先 `onBeforeSaveVersion` flush autoSave、再 `createCheckpoint`、最后 toast
- [ ] `cargo test -p mango-core` 至少新增 4 个 checkpoint 测试
- [ ] `cargo clippy -p mango-core --all-targets -- -D warnings` 干净
- [ ] `pnpm test:run` 至少新增 15 个前端测试（shortcuts 6 + toolbar 5 + temporal 6 + checkpoint hook 1，按上面拆分）
- [ ] `pnpm typecheck` 干净
- [ ] ts-rs 重新生成的 `EpisodeCheckpoint.ts` / `CreateCheckpointInput.ts` 含正确字段；`commands.ts` diff 仅含 1 个新命令
- [ ] 手动验证清单全过

## 依赖关系

- **前置**：
  - spec-21（`canvasStore` 必须先存在才能套 middleware；`useAutoSaveLayout` 已建才能加 flush 方法）
  - spec-22（业务节点已建，撤销/重做才有实际操作对象）
  - spec-23（拖放先合，避免 zundo 套上后再追加 `addNode` 是否进栈的回炉 review；拖放产生的 `addNode` **按默认进栈**——见 §"Undo 与业务副作用的边界"表）
- **平行**：无
- **后续**：
  - MS4 将本 spec 创建的 `episode_checkpoint` 行扩展为完整版本管理（list / restore / 自动清理 / SQLite backup）；本 spec 已对齐表 schema，MS4 无需迁移

## 与原任务文档的差异

合并自原任务 11（Undo/Redo）+ 任务 12（画布工具栏）。

- **原任务 11 没明确"撤销是否反向写关联表"**：本 spec 决议「不反向」并写明理由与代价
- **原任务 11 没说 viewport 是否进 undo 栈**：本 spec 决议「不进」（`partialize` 排除）—— 拖画布不应该被 Ctrl+Z 撤销，这是常见 IDE 约定
- **原任务 11 没说 init 加载是否进栈**：本 spec 决议「不进」（`init/reset` 用 `pause/resume + clear`）—— 否则首次进画布按一下 Cmd+Z 就清空
- **原任务 11 没说节点拖动期防抖**：本 spec 决议「用 `onNodeDragStart/Stop` 手动 pause/resume」—— 比 `equality` 或 `handleSet` 节流更直观
- **原任务 11 没说 macOS Cmd+Y**：本 spec 决议「macOS 不接受 Cmd+Y」—— 符合系统约定
- **原任务 12 的"保存版本"含义模糊**：本 spec 明确 MS3 范围内是「占位」—— DB 写一行，但 list/restore 等完整功能属 MS4
- **原任务 12 没提工具栏与 React Flow zoom 状态同步**：本 spec 用 `useStore` hook 实时读 `transform[2]`，避免 stale zoom
- **原任务 12 没提 flush autoSave 与 checkpoint 的顺序**：本 spec 明确"先 flush → 再 createCheckpoint"，否则 checkpoint 读到的可能是上一个 debounce 周期的 layout
- **原任务 12 假设 episode_checkpoint 表是简单 snapshot 列**：实际 schema 是分列存储（canvas_nodes_json / canvas_edges_json / canvas_viewport_json），且有 `version_number NOT NULL` / `trigger_type CHECK` 约束。本 spec 字段名严格对齐已有 schema，**不改 migration**
