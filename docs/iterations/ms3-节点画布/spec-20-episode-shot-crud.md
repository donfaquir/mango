# SPEC-20: Episode 与 Shot CRUD

## 概述

补齐 MS3 节点画布的**数据基础**。`episode` / `shot` / `shot_character` / `shot_scene` / `shot_prop` 五张表已在 MS0 `001_initial.sql` 全量定义，但截至 2026-05-27 没有任何 Rust models / queries / Tauri commands / 前端页面落地。**画布按 episode 加载**（spec-21）、**节点关联 shot**（spec-22）、**任务可绑定 shot_id**（MS2 已支持，但 UI 还没法选 shot），这些都依赖本 spec 把数据层与最小 UI 打通。

本 spec **不引入新表、不引入新依赖**——纯粹是把 schema 已存在的实体接上骨架。

> ⚠️ **破坏性变更**：`crates/core/src/lib.rs` 的 `models/` 与 `db/queries/` 新增 `episode.rs` / `shot.rs`；`src-tauri/src/commands/` 新增 `episode.rs` / `shot.rs`；`make_builder()` collect_commands! 列表新增 12+ 个命令；前端 `WorkspaceNav` 新增「剧本」入口。

## 设计目标 / 非目标

**目标**：

- `Episode` / `Shot` 两个 model 加 `serde + specta::Type`，导出到前端 bindings
- `db::queries::episode`：create / get / list_by_project / update / delete / reorder
- `db::queries::shot`：create / get / list_by_episode / update / delete / reorder
- `shot_character` / `shot_scene` / `shot_prop` 三张关联表的 link / unlink / list_by_shot 操作（spec-22 角色→分镜连线需要写入这些表）
- Tauri commands 12 个：episode 5 + shot 5 + shot 关联 2
- 前端最小 UI：`EpisodeListPage`（按 project 列出 episode，可建/改名/排序）+ `ShotListPanel`（episode 下的 shot 列表，可建/改/排序）。**本 spec 不实现画布，但实现「无画布也能管理剧本与分镜」的基础界面**
- `WorkspaceNav` 增加「剧本」入口（位于「主体库」与「素材库」之间），点击进入 EpisodeListPage

**非目标**（明确划归后续 spec）：

- 节点画布渲染 / CanvasLayout 持久化 → spec-21
- 5 种业务节点 / 自定义 Edge → spec-22
- 双路拖放 → spec-23
- Undo/Redo / 工具栏 → spec-24
- shot 的 `image_prompt` / `video_prompt` 自动生成（如 LLM 协助）—— V2 范围
- episode_checkpoint 的创建/恢复——MS4

## 技术方案

### 目录/文件结构

```text
crates/core/src/
├── lib.rs                              # （无变化，models/db 都已在）
├── models/
│   ├── mod.rs                          # +pub mod episode; +pub mod shot;
│   ├── episode.rs                      # 新增：Episode + CreateEpisodeInput + UpdateEpisodeInput
│   └── shot.rs                         # 新增：Shot + ShotStatus + Create/Update Input + Shot{Character,Scene,Prop}Link
└── db/queries/
    ├── mod.rs                          # +pub mod episode; +pub mod shot;
    ├── episode.rs                      # 新增：CRUD + reorder_within_project
    └── shot.rs                         # 新增：CRUD + reorder_within_episode + 关联表 link/unlink/list

src-tauri/src/
├── lib.rs                              # collect_commands! 增加 12 项
└── commands/
    ├── mod.rs                          # +pub mod episode; +pub mod shot;
    ├── episode.rs                      # create/get/list/update/delete/reorder
    └── shot.rs                         # create/get/list/update/delete/reorder
                                          + link_character/unlink_character (类似 scene/prop)

src/
├── routes.tsx                          # /project/:id/episodes 路由 + /episodes/:episodeId/shots 路由
├── pages/
│   ├── EpisodeListPage.tsx             # 新增
│   └── EpisodeDetailPage.tsx           # 新增（含 ShotListPanel）
├── components/
│   ├── layout/WorkspaceNav.tsx         # 新增「剧本」tab，置于「主体库」之后
│   └── episodes/                       # 新建子目录
│       ├── EpisodeCard.tsx
│       ├── CreateEpisodeDialog.tsx
│       ├── ShotListPanel.tsx
│       ├── ShotCard.tsx
│       └── CreateShotDialog.tsx
└── hooks/
    ├── useEpisodes.ts                  # 新增：useEpisodeList / useCreateEpisode / useUpdateEpisode / useDeleteEpisode / useReorderEpisodes
    └── useShots.ts                     # 新增：useShotList / useCreate/Update/Delete/Reorder + useShotLinks
```

### 依赖项变更

**Rust**：无新增。
**Tauri 插件**：无新增。
**npm**：无新增。

### 数据模型

`crates/core/src/models/episode.rs`：

```rust
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Episode {
    pub id: String,
    pub project_id: String,
    pub title: String,
    #[specta(type = specta_typescript::Number)]
    pub order_index: i64,
    pub script_text: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Type)]
pub struct CreateEpisodeInput {
    pub project_id: String,
    pub title: String,
    #[serde(default)]
    pub script_text: Option<String>,
}

#[derive(Debug, Deserialize, Type)]
pub struct UpdateEpisodeInput {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub script_text: Option<String>,
}
```

`crates/core/src/models/shot.rs`：

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ShotStatus {
    Draft,
    Ready,
    Generating,
    Done,
}

impl ShotStatus {
    pub fn as_db_str(self) -> &'static str { /* ... */ }
    pub fn from_db_str(s: &str) -> Option<Self> { /* ... */ }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Shot {
    pub id: String,
    pub episode_id: String,
    #[specta(type = specta_typescript::Number)]
    pub order_index: i64,
    pub summary: String,
    pub duration_sec: Option<f64>,
    pub camera_angle: String,
    pub shot_type: String,
    pub mood: String,
    pub dialogue: String,
    pub video_prompt: String,
    pub image_prompt: String,
    pub status: ShotStatus,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Type)]
pub struct CreateShotInput {
    pub episode_id: String,
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Deserialize, Type, Default)]
pub struct UpdateShotInput {
    pub id: String,
    #[serde(default)] pub summary: Option<String>,
    #[serde(default)] pub duration_sec: Option<f64>,
    #[serde(default)] pub camera_angle: Option<String>,
    #[serde(default)] pub shot_type: Option<String>,
    #[serde(default)] pub mood: Option<String>,
    #[serde(default)] pub dialogue: Option<String>,
    #[serde(default)] pub video_prompt: Option<String>,
    #[serde(default)] pub image_prompt: Option<String>,
    #[serde(default)] pub status: Option<ShotStatus>,
}

/// shot_character / shot_scene / shot_prop 三张关联表的统一返回结构
#[derive(Debug, Clone, Serialize, Type)]
pub struct ShotLinks {
    pub character_ids: Vec<String>,
    pub scene_ids: Vec<String>,
    pub prop_ids: Vec<String>,
}
```

> **设计说明**：`CreateShotInput` 故意只接受 `episode_id` + `summary`——其余字段（camera_angle / mood / dialogue / image_prompt 等）通过后续 `update` 编辑。这避免「建分镜」这一高频动作被一个 10 字段大表单挡住；spec-22 在画布上「拖出一个分镜节点」的交互也只填 summary 默认即可。

### Rust 查询层

`crates/core/src/db/queries/episode.rs`：

```rust
const SELECT_COLUMNS: &str = "id, project_id, title, order_index, script_text, created_at, updated_at";

pub fn create(conn: &Connection, input: &CreateEpisodeInput) -> Result<Episode> {
    let id = uuid::Uuid::new_v4().to_string();
    // order_index = MAX(order_index) + 1 within project
    let next_order: i64 = conn.query_row(
        "SELECT COALESCE(MAX(order_index), -1) + 1 FROM episode WHERE project_id = ?1",
        params![input.project_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO episode (id, project_id, title, order_index, script_text) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, input.project_id, input.title, next_order, input.script_text.as_deref().unwrap_or("")],
    )?;
    get_by_id(conn, &id)
}

pub fn list_by_project(conn: &Connection, project_id: &str) -> Result<Vec<Episode>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM episode WHERE project_id = ?1 ORDER BY order_index ASC"
    ))?;
    let rows = stmt.query_map(params![project_id], row_to_episode)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// 批量重排：传入有序的 episode_id 列表，按列表顺序写入 order_index。
/// 用同一事务保证一致性。
pub fn reorder_within_project(conn: &mut Connection, project_id: &str, ordered_ids: &[String]) -> Result<()> {
    let tx = conn.transaction()?;
    for (idx, id) in ordered_ids.iter().enumerate() {
        let n = tx.execute(
            "UPDATE episode SET order_index = ?1, updated_at = datetime('now') WHERE id = ?2 AND project_id = ?3",
            params![idx as i64, id, project_id],
        )?;
        if n == 0 {
            return Err(CoreError::NotFound { entity: "episode", id: id.clone() });
        }
    }
    tx.commit()?;
    Ok(())
}

// update / delete / get_by_id 标准实现，省略
```

`crates/core/src/db/queries/shot.rs` 同款，外加关联表：

```rust
pub fn list_links(conn: &Connection, shot_id: &str) -> Result<ShotLinks> {
    let character_ids = collect_ids(conn, "SELECT character_id FROM shot_character WHERE shot_id = ?1", shot_id)?;
    let scene_ids = collect_ids(conn, "SELECT scene_id FROM shot_scene WHERE shot_id = ?1", shot_id)?;
    let prop_ids = collect_ids(conn, "SELECT prop_id FROM shot_prop WHERE shot_id = ?1", shot_id)?;
    Ok(ShotLinks { character_ids, scene_ids, prop_ids })
}

pub fn link_character(conn: &Connection, shot_id: &str, character_id: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO shot_character (shot_id, character_id) VALUES (?1, ?2)",
        params![shot_id, character_id],
    )?;
    Ok(())
}

pub fn unlink_character(conn: &Connection, shot_id: &str, character_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM shot_character WHERE shot_id = ?1 AND character_id = ?2",
        params![shot_id, character_id],
    )?;
    Ok(())
}

// link_scene / unlink_scene / link_prop / unlink_prop 同款
```

> **设计说明**：`INSERT OR IGNORE` 避免重复连线时报 PK 冲突——画布上同一个连线可能被用户重新拖一次，幂等是最合理的行为。

### Tauri commands

`src-tauri/src/commands/episode.rs`：

```rust
#[tauri::command]
#[specta::specta]
pub async fn create_episode(state: State<'_, AppState>, input: CreateEpisodeInput) -> Result<Episode, IpcError> {
    state.db.call(move |c| episode_q::create(c, &input).map_err(Into::into)).await.map_err(IpcError::from)
}

#[tauri::command] #[specta::specta]
pub async fn list_episodes(state: State<'_, AppState>, project_id: String) -> Result<Vec<Episode>, IpcError> { /* ... */ }

#[tauri::command] #[specta::specta]
pub async fn get_episode(state: State<'_, AppState>, id: String) -> Result<Episode, IpcError> { /* ... */ }

#[tauri::command] #[specta::specta]
pub async fn update_episode(state: State<'_, AppState>, input: UpdateEpisodeInput) -> Result<Episode, IpcError> { /* ... */ }

#[tauri::command] #[specta::specta]
pub async fn delete_episode(state: State<'_, AppState>, id: String) -> Result<(), IpcError> { /* ... */ }

#[tauri::command] #[specta::specta]
pub async fn reorder_episodes(state: State<'_, AppState>, project_id: String, ordered_ids: Vec<String>) -> Result<(), IpcError> { /* ... */ }
```

`src-tauri/src/commands/shot.rs` 同款 6 个 + 关联表：

```rust
#[tauri::command] #[specta::specta]
pub async fn list_shot_links(state: State<'_, AppState>, shot_id: String) -> Result<ShotLinks, IpcError> { /* ... */ }

#[tauri::command] #[specta::specta]
pub async fn link_shot_subject(
    state: State<'_, AppState>,
    shot_id: String,
    subject_id: String,
    subject_kind: SubjectKind, // 复用 src/components/subjects/types.ts 同名枚举
) -> Result<(), IpcError> { /* dispatch to link_character/scene/prop */ }

#[tauri::command] #[specta::specta]
pub async fn unlink_shot_subject(state: State<'_, AppState>, shot_id: String, subject_id: String, subject_kind: SubjectKind) -> Result<(), IpcError> { /* ... */ }
```

> **设计说明**：把 link_character / link_scene / link_prop **不拆三个 IPC 命令**，统一为 `link_shot_subject(kind)` —— 前端只需要一个 mutation，画布上的连线代码 (spec-22) 也是「连个角色就调 link_shot_subject('character')」。把 subject 类型作为参数避免 IPC 命令数量膨胀。

注册到 `make_builder()`：episode 6 + shot 6 + shot links 3 = 共 15 个新命令。

### 前端

#### 路由扩展

`src/routes.tsx` 在 `project/:projectId` 下加：

```tsx
{
  path: "episodes",
  children: [
    { index: true, element: <EpisodeListPage /> },
    {
      path: ":episodeId",
      element: <EpisodeDetailPage />,
    },
  ],
},
```

#### WorkspaceNav 顺序

```ts
const navItems = [
  { path: "subjects/character", label: "主体库", icon: Users },
  { path: "episodes", label: "剧本", icon: BookOpen },   // 新增
  { path: "assets", label: "素材库", icon: FolderOpen },
  { path: "generation", label: "AI 生成", icon: Wand2 },
];
```

#### EpisodeListPage 骨架

```tsx
export default function EpisodeListPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const episodes = useEpisodeList(projectId!);
  const [dialogOpen, setDialogOpen] = useState(false);

  return (
    <div className="flex h-full flex-col gap-4 p-6">
      <header className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold">剧本与分镜</h1>
        <Button onClick={() => setDialogOpen(true)}>新建一集</Button>
      </header>
      {episodes.isLoading ? <Skeleton /> : null}
      {episodes.data?.length === 0 ? <EmptyState ... /> : null}
      <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
        {episodes.data?.map((ep) => <EpisodeCard key={ep.id} episode={ep} />)}
      </div>
      <CreateEpisodeDialog open={dialogOpen} onOpenChange={setDialogOpen} projectId={projectId!} />
    </div>
  );
}
```

#### EpisodeDetailPage 骨架

`/project/:projectId/episodes/:episodeId` —— 三段布局：

```
┌──────────────────────────────────────────┐
│  Episode 标题 + 操作（改名 / 删除 / 跳画布） │
├──────────────────────────────────────────┤
│  ScriptEditor                             │  ← 简单 <textarea>，失焦保存 episode.script_text
├──────────────────────────────────────────┤
│  ShotListPanel                            │  ← shot 卡片列表，可建/删/拖排序
└──────────────────────────────────────────┘
```

「跳画布」按钮在 spec-21 才真正连到 `/project/:id/episodes/:episodeId/canvas`，本 spec 占位为 disabled tooltip「画布即将上线」。

#### 拖拽排序

EpisodeCard / ShotCard 用 `react-beautiful-dnd` 或 HTML5 drag —— 本 spec **不引入新依赖**，用 HTML5 drag handle + `pointer-events: none` 子元素 + `useReorderEpisodes` mutation 提交新顺序。如果交互过于粗糙，spec-22 / spec-23 实施时回头优化（届时 dnd-kit 的引入已经在画布场景中确定）。

### 错误场景

| 场景 | 行为 |
|---|---|
| `create_episode` 时 project_id 不存在 | FK 触发 `CoreError::Sqlite`；IPC 返回 IpcError，UI toast「项目不存在」 |
| `delete_episode` 后 shot/canvas_layout 全被级联删除 | `001_initial.sql` 已有 `ON DELETE CASCADE`，行为正确；UI 二次确认 dialog |
| `reorder_episodes` 列表中某 id 不属于该 project | `reorder_within_project` 返回 `NotFound`；整事务回滚，前端 toast 错 |
| `update_shot` 把 `status` 改为 `done` 但没有关联 asset | 本 spec 不校验业务规则；MS3 范围内 status 字段只是颜色标记 |
| 两个 episode 同 `order_index`（数据手工污染） | `list_by_project` 排序结果不稳定，但不报错；用户重排一次即可愈合 |
| `link_shot_subject` 时 subject_id 与 subject_kind 不匹配（如把 character_id 传成 prop） | FK 触发 `CoreError::Sqlite`；IPC 返回 IpcError |

### 测试策略

#### Rust 单元测试

`crates/core/src/db/queries/episode.rs`：
- `create` → `get_by_id` 字段完整往返
- `create` 自动分配 order_index = MAX + 1（用 :memory: + 3 条插入断言 0/1/2）
- `list_by_project` 按 order_index 升序
- `update` 部分字段（title 改、script_text 不变）正确合并
- `delete` 级联清理 shot / canvas_layout（INSERT 一条 canvas_layout → delete episode → 验证消失）
- `reorder_within_project` 三个 id 重排 → 列表顺序变化；其中一个 id 不属于项目 → 整事务回滚

`crates/core/src/db/queries/shot.rs`：
- 同款 CRUD
- `list_links` 空时返回三个空 Vec
- `link_character` 幂等（连续两次 INSERT OR IGNORE 不报错；count 仍为 1）
- `link_shot_subject` 三种 kind 分别 dispatch 正确

#### 前端测试

- `useEpisodes.test.ts`：mock commands → useEpisodeList / useCreateEpisode 正确触发 invalidateQueries
- `EpisodeCard.test.tsx`：渲染 title、order_index、shot 计数
- `CreateEpisodeDialog.test.tsx`：title 空校验、submit 调用 mutation

#### 手动验证清单

- [ ] `pnpm tauri dev` 进项目 → 点击「剧本」tab → 看到 EpisodeListPage 空态
- [ ] 新建一集「第一幕：开学」→ 看到 EpisodeCard
- [ ] 点进 EpisodeDetailPage → 编辑 script_text 失焦 → 重启应用 → 文本保留
- [ ] 在 EpisodeDetailPage 下方 ShotListPanel 建 3 个分镜 → 可拖动排序
- [ ] 删一集 → 该集的 shot 全部消失（DB 验证 `SELECT * FROM shot WHERE episode_id = '...'` 返回空）

## 验收标准

- [ ] `crates/core/src/models/{episode,shot}.rs` 完整定义 + specta 导出
- [ ] `crates/core/src/db/queries/{episode,shot}.rs` 全套 CRUD + reorder + 关联表 link/unlink
- [ ] 15 个 Tauri commands 注册到 `make_builder()`
- [ ] `cargo test -p mango-core` 新增至少 14 个单测全绿（episode 6 + shot 8）
- [ ] `cargo clippy -p mango-core --all-targets -D warnings` 干净
- [ ] `src/routes.tsx` 新增 `/episodes` 路由树
- [ ] `WorkspaceNav` 新增「剧本」tab，置于「主体库」之后
- [ ] `EpisodeListPage` / `EpisodeDetailPage` / `ShotListPanel` 完整实现
- [ ] `useEpisodes` / `useShots` hook 文件覆盖所有 IPC 命令
- [ ] `pnpm test:run` 至少新增 5 个前端测试
- [ ] `pnpm typecheck` 干净
- [ ] `cargo test -p mango-tauri export_bindings` 重新生成 commands.ts，diff 仅含新增字段/命令

## 依赖关系

- **前置**：MS1 全部 spec（特别是 spec-10 项目数据目录、spec-11 主体 CRUD —— 关联表用到 character/scene/prop 主键）
- **平行**：无（spec-20 本身不依赖其他 MS3 spec）
- **后续**：
  - SPEC-21 画布按 `episode_id` 加载 CanvasLayout；EpisodeDetailPage 的「跳画布」按钮在 spec-21 启用
  - SPEC-22 节点点击「关联角色」时调用 `link_shot_subject`
  - SPEC-22 的 `StoryboardNode` 显示的字段直接来自 `shot.{summary,status,...}`
  - 已经完成的 MS2 SubmitTaskPanel 当前 shot_id 始终为 null；本 spec 完成后可扩展支持「为某个 shot 提交生成任务」（但 SubmitTaskPanel 改造不在本 spec 范围）

## 与原任务文档的差异

原 `开发任务.md` 的 12 个任务**没有任何一项**覆盖 episode/shot 数据层——隐式假设它已经存在。本 spec 把这块缺失的「数据基础」从 MS3 拆出独立 spec，理由：

1. 没有 episode/shot CRUD 就无法构造画布上下文（spec-21 卡死）
2. 数据层的 PR 评审节奏与画布 UI 完全不同，分开评审更高效
3. EpisodeListPage / ShotListPanel 这层「无画布也能管理剧本」的最小界面本身就有价值——给只想看分镜表格、不想拖画布的用户一条逃生路

把这一 spec 放在 MS3 而不是回填 MS1，是因为：
- MS1 的范围是「主体库 + 账号」（创作前置素材），不含「剧本/分镜」（创作流程实体）
- MS3 的核心交付物是「画布编排」，把数据基础放在第一 spec 与画布 spec 紧邻，符合「依赖关系」的连贯性
