# MS3 新增依赖一览

本文档汇总 MS3 五个 spec（20/21/22/23/24）中提议引入的所有新依赖，便于一次性评审与批准。**版本号在实施 PR 时通过 `WebSearch` / `WebFetch crates.io` / `WebFetch npmjs.com` 核对当时最新 stable 后再 pin**（项目规则 `general.md` §"Dependency & Version Constraints"）。

> 当前会话受网络限制无法访问 npmjs.com / crates.io，因此本表所有版本号留 `?` 占位。实施 PR **必须** 完成 §6 的版本验证清单后再写入 `package.json` / `Cargo.toml`。

---

## 1. npm 依赖（`package.json`）

| 包 | 用途 | 引入 spec | 备注 |
|---|---|---|---|
| `@xyflow/react` | React Flow 12，画布渲染核心 | spec-21 | 这是 React Flow 11.x → 12.x 改名后的官方包，提供 hooks + 内置 minimap/controls/background |
| `zundo` | Zustand temporal middleware，实现 undo/redo 栈 | spec-24 | 比手写 history 栈简单；可配置 `limit`、`equality`、`partialize` |
| `use-debounce` | CanvasLayout 自动保存的 1s 防抖 hook | spec-21 | 备选：自写 setTimeout + ref；本项目已经有多处防抖需求（spec-12 文件导入），值得引入统一封装 |

`@xyflow/react` 12.x 自带类型定义，无需 `@types/*`。它内部依赖 `d3-zoom`、`d3-drag`、`zustand`（已有），不需要手动声明。

### 1.1 spec-22 / spec-23 / spec-24 不新增 npm 包

- 节点 / Edge 自定义组件用 `@xyflow/react` 提供的 `NodeProps` / `EdgeProps`（spec-22）
- HTML5 DnD 是浏览器原生 API（spec-23 §"内部面板拖放"）
- 系统文件拖入用 Tauri 2 `getCurrentWebviewWindow().onDragDropEvent`（`@tauri-apps/api` 已有，spec-23 §"系统文件拖放"）
- 工具栏图标用 `lucide-react`（已有）

### 1.2 shadcn/ui 组件（`pnpm dlx shadcn@latest add ...`）

| 组件 | 用途 | 引入 spec |
|---|---|---|
| `tooltip` | 工具栏按钮 hover 提示 | spec-24 |
| `separator` | 工具栏按钮分组分隔线（垂直） | spec-24 |
| `context-menu` | 节点 / Edge 右键菜单（删除、查看详情） | spec-22, spec-24 |

实施时 `pnpm dlx shadcn@latest add tooltip separator context-menu`。

---

## 2. Rust 依赖（workspace 根 `Cargo.toml`）

| crate | 用途 | 引入 spec | 备注 |
|---|---|---|---|
| 无新增 | episode/shot/canvas_layout 的 CRUD 完全复用 MS1/MS2 已有的栈（rusqlite + tokio-rusqlite + serde + specta） | spec-20, spec-21 | — |

### 2.1 编辑器侧依赖（仅 frontend）

spec-21 把 `canvas_layout.{nodes_json,edges_json,viewport_json}` 当作不透明字符串往返于前后端 —— Rust 侧不解析 JSON 结构，只校验「是合法 JSON」（用 `serde_json::from_str::<serde_json::Value>(...)?`）。这避免了 Rust 与 React Flow 节点 schema 同步演进的负担。

---

## 3. Tauri 插件 / src-tauri 增量

无新增 Tauri 插件。

- 系统文件拖入：Tauri 2 的 `tauri::DragDropEvent` 通过 `WebviewWindow::on_drag_drop_event` 在 webview 层暴露给前端，**不需要** `tauri-plugin-fs` 或 `tauri-plugin-dialog` 增量
- 文件路径校验（白名单 + scope）：复用 MS1 spec-10 的 `paths::project_root` + spec-12 的 `import_asset`
- spec-23 复用既有 `useGlobalDropTarget`（spec-12 引入），不再新挂 webview-global listener；`DropHandler` 签名从 `(paths) => void` 向后兼容扩展为 `(paths, position) => void`

### 3.1 Tauri command 增量（一览）

| command | 引入 spec | 入参 | 出参 | 用途 |
|---|---|---|---|---|
| `link_shot_subject` | spec-22 | `(shot_id, subject_id, subject_kind)` | `()` | character_to_shot 边落地时双写 `shot_character` 表 |
| `unlink_shot_subject` | spec-22 | `(shot_id, subject_id, subject_kind)` | `()` | 边删除 / 节点删除时反向 unlink |
| `assign_asset_to_shot` | spec-23 | `(id, shot_id: Option<String>)` | `Asset` | 拖 asset 到 storyboard 节点上时写 `asset.shot_id`；`None` = 解绑 |
| `create_episode_checkpoint` | spec-24 | `CreateCheckpointInput { episode_id, label? }` | `EpisodeCheckpoint` | 「保存版本」按钮：从 DB 读 canvas_layout 快照，写入 `episode_checkpoint` 一行；`version_number` 按 episode 自增；`trigger_type` 固定 `'manual'` |

所有新 command 走 `tauri-specta` + `ts-rs`，自动生成到 `src/lib/bindings/commands.ts`。

---

## 4. 数据库 migration

| migration | 内容 | 引入 spec |
|---|---|---|
| 无 | episode / shot / canvas_layout / episode_checkpoint / shot_character / shot_scene / shot_prop 七张表全部已在 `001_initial.sql` | — |

MS3 **不写新 migration**。如果在 spec-22 实施过程中发现节点 schema 需要额外字段（如 `shot.canvas_position_x/y` 单独列存位置），由当时再决定是否切回「画布 layout JSON 单点存储」策略；本 MS3 优先走「layout JSON 全量存」。

---

## 5. 文件系统/平台权限影响

- spec-21 持久化：写入流量集中在 `canvas_layout` 表，单 episode 一行 UPSERT；50+ 节点的 JSON 序列化字符串估计 30~80KB，SQLite 单次 UPDATE 毫秒级
- spec-23 系统文件拖入：复用 MS1 spec-12 的 `import_asset` IPC，沙箱权限继承（macOS 需要在沙箱配置里 `com.apple.security.files.user-selected.read-write`，但 MS1 已配置）
- spec-23 内部拖放：纯前端事件，无 IPC 流量

---

## 6. 版本验证清单（实施 PR 时执行）

实施前，每项依赖按以下流程定版：

1. `WebSearch "<package-name> latest version npmjs.com"` 或 `WebFetch https://www.npmjs.com/package/<name>` 拿到当前 stable
2. 与 `docs/技术选型.md` 既定版本比对（若已列入）
3. 与 `package.json` 中已存在的间接依赖兼容性比对（特别是 `@xyflow/react` 是否要求最低 React 版本）
4. `@xyflow/react` 12.x：必须 React 17+，本项目 React 19 满足；TypeScript 5.x+，本项目 TS 6 满足
5. `zundo`：必须 zustand v5+，本项目已 pin zustand 5，满足
6. 在 spec 实施 PR 描述中记录所选版本与依据

不依赖训练数据；不接受「我记得是 X 版本」的笼统说法。

---

## 7. 风险与未决项

- **`@xyflow/react` 12.x 的 `viewport` 受控/非受控切换**：spec-21 设计为「打开时非受控初始化 → 用户操作 → onMoveEnd 写库」。如果实测出现 viewport 闪烁（受控值与内部状态争抢），需要切回 `defaultViewport` + 手动 `useReactFlow().setViewport` 在加载时调用
- **Undo/Redo 栈与持久化的耦合**：zundo 默认对 store 全部状态做 snapshot。spec-24 必须用 `partialize` 仅追踪 `{nodes, edges}` —— 否则 viewport 拖动、临时 UI 状态都会进 undo 栈，体验差
- **Undo 栈被 init / drag 中间帧污染**：spec-24 决议 `init/reset` 用 `pause/resume + clear` 包裹（否则首次进画布按 Cmd+Z 就清空）；节点拖动期用 React Flow `onNodeDragStart/Stop` 手动 pause/resume（否则一次拖动产生几十帧 snapshot，Cmd+Z 一次只能撤一像素）。zundo 2.3 已具备 `pause/resume/clear` API
- **`episode_checkpoint` schema 严格对齐 `001_initial.sql`**：实际表是分列存储（`canvas_nodes_json` / `canvas_edges_json` / `canvas_viewport_json`）且 `version_number INTEGER NOT NULL`、`trigger_type CHECK('auto','manual')`。spec-24 早期稿用 `snapshot`/`note` 是错的；实施时务必按 spec-24 §"episode_checkpoint 表实际 schema" 写 INSERT，**不改 migration**
- **大画布性能**（节点 > 50）：spec-21 §"性能优化" 提到 Web Worker 序列化。优先级低于本里程碑的核心功能，建议在 spec-21 实施 PR 中**先不做** Worker，先用 `requestIdleCallback` 把 JSON.stringify 推到空闲帧；性能不达标再升级到 Worker。决策档案：见 spec-21 §"性能优化决策树"
- **EpisodeCheckpoint 创建/恢复**：spec-24 工具栏的「保存版本」按钮**仅 emit 事件 + 占位 Tauri command**，真正的 checkpoint 业务由 MS4 实施。MS3 范围内点击该按钮只在 DB 写入一条最小化 checkpoint 行（不实现恢复 / 列表 / 自动清理）
