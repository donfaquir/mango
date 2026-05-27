# SPEC-10: 项目数据目录策略

## 概述

为 MS1 引入"项目目录"概念。每个项目在文件系统上拥有独立目录，包含 `assets/` 和 `thumbnails/` 子目录，由用户在创建项目时选择位置（GUI 默认提供约定路径建议）。该目录是后续所有素材文件、缩略图、导出成品的存放根。

本 spec 是 MS1 任务 7（素材导入管线）和任务 5/6（主体页面，含参考图预览）的前置依赖；不解决该问题，MS1 后续任务无法落地。

> ⚠️ **破坏性变更**：`CreateProjectInput` 新增 `root_path: Option<String>` 字段，`Project` 新增 `root_path: String` 字段。所有调用方需同步更新。MS0 已合并代码 + 测试数据库中可能存在 NULL 行，本 spec 通过启动期 backfill 一次性补齐。

## 设计目标 / 非目标

**目标**：
- `project` 表新增 `root_path` 字段（列允许 NULL 仅为 ALTER TABLE 兼容，应用层从 MS1 起永不写 NULL）
- 项目创建流程（GUI 默认建议 + 用户可改 / CLI 可选）
- 创建时自动建立 `<root>/assets/` 和 `<root>/thumbnails/` 子目录
- 提供 `mango_core::paths` 路径解析模块，所有 IO 路径计算走这一处
- 既有项目（MS0 调试期产生的 NULL root_path）启动时自动回填到约定路径；回填失败硬性失败启动（不留 NULL 行被运行时代码踩到）
- 提供 `mango_core::startup::initialize` 统一启动钩子，**Tauri 与 CLI 共用同一入口**

**非目标**（V2 或后续 MS）：
- 切分 per-project SQLite（`project.db`）—— 主体数据继续存在共享 `mango.db`
- 修改已存在项目的 `root_path`（"项目移动"功能）
- 项目目录链接断裂的可视化修复 UI（V2）
- 项目导出/打包（zip 整个目录用于分享）

## 技术方案

### 目录/文件结构

```
crates/core/src/
├── lib.rs                               # 添加 pub mod paths; pub mod startup;
├── paths.rs                             # 路径解析与目录布局
├── startup.rs                           # 启动期统一入口（backfill + seed）
├── error.rs                             # 新增 CoreError::Io 变体
├── models/project.rs                    # Project 增加 root_path 字段
├── db/
│   ├── migrator.rs                      # 注册 002 迁移
│   ├── migrations/
│   │   ├── 001_initial.sql
│   │   └── 002_add_project_root_path.sql   # 新增
│   └── queries/project.rs               # create/update 处理 root_path
src-tauri/src/
├── commands/
│   ├── project.rs                       # create_project 接受可选 root_path
│   └── dialog.rs                        # 新增：pick_project_directory / suggest_project_root
└── lib.rs                               # 启动时调 startup::initialize
src-tauri/capabilities/default.json      # 加 dialog:allow-open
src/pages/ProjectListPage.tsx            # 创建对话框加目录选择器
```

### 依赖项

**Rust（新增，需批准）**：
```toml
# workspace 根 Cargo.toml
slug = "0.1"                       # 用于项目名生成默认目录名（仅 GUI 建议路径展示）
```

**Tauri 插件（新增，需批准）**：
```toml
# src-tauri/Cargo.toml
tauri-plugin-dialog = "2"
```

> ⚠️ 实施前用 `WebSearch` 核对 `tauri-plugin-dialog` 与 `slug` 的当时最新 stable 版本。`slug` 仅用于 `suggest_project_root` 生成可读默认名；真正的存储路径以 `project.id`（uuid）为底，避免重名冲突——见下文。

`src-tauri/src/lib.rs` 注册插件：
```rust
.plugin(tauri_plugin_dialog::init())
```

### 数据模型变更

**`002_add_project_root_path.sql`**：
```sql
ALTER TABLE project ADD COLUMN root_path TEXT;
```

> **设计说明**：不加 `NOT NULL` 约束。SQLite 的 `ALTER TABLE ADD COLUMN` 加 NOT NULL 必须给默认值，但这里没有合理的静态默认（不同安装的 `app_data_dir` 不同）。NULL 仅作为 ALTER TABLE 的临时态由启动期 backfill 立即兜底；应用层从 MS1 起永不写入 NULL，运行时代码可安全把 `root_path` 视作 String 非空。

**`models/project.rs`** 变更：
```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: String,
    pub style_prompt: String,
    pub root_path: String,                 // 新增；启动期 backfill 之后保证非空
    #[specta(type = Option<specta_typescript::Number>)]
    pub global_seed: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Type)]
pub struct CreateProjectInput {
    pub name: String,
    /// None 表示走"约定路径"（`<app_data>/projects/{uuid}/`）。
    /// Some(s) 表示用户显式选择的绝对路径；走 §"路径校验"。
    #[serde(default)]
    pub root_path: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub style_prompt: Option<String>,
    #[serde(default)]
    #[specta(type = Option<specta_typescript::Number>)]
    pub global_seed: Option<i64>,
}
```

> **设计说明**：`root_path` 设为可选，理由是 GUI / CLI 两种入口语义不同——GUI 总能调起目录选择器拿到值，但 CLI 用户更想要"不指定就丢到默认位置"的 ergonomics。core 层把这两种路径统一为同一处分支。`UpdateProjectInput` 不暴露 `root_path`——MS1 范围内禁止改。

### Rust 错误类型扩展

`crates/core/src/error.rs`：
```rust
#[derive(Error, Debug)]
pub enum CoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),                 // 新增

    #[error("not found: {entity} with id '{id}'")]
    NotFound { entity: &'static str, id: String },

    #[error("validation error: {0}")]
    Validation(String),
}
```

> 后续 spec-12（asset import）和 spec-13（keyring）都依赖 `Io` 变体，避免每处都自己 `.map_err(|e| Validation(...))`。

### 路径解析模块（`crates/core/src/paths.rs`）

```rust
use std::path::{Path, PathBuf};

pub const ASSETS_SUBDIR: &str = "assets";
pub const THUMBNAILS_SUBDIR: &str = "thumbnails";
pub const PROJECTS_SUBDIR: &str = "projects";

/// 项目根目录下的素材目录绝对路径
pub fn assets_dir(project_root: &Path) -> PathBuf {
    project_root.join(ASSETS_SUBDIR)
}

/// 项目根目录下的缩略图目录绝对路径
pub fn thumbnails_dir(project_root: &Path) -> PathBuf {
    project_root.join(THUMBNAILS_SUBDIR)
}

/// 创建项目目录布局（root + assets + thumbnails）。幂等。
pub fn ensure_project_layout(project_root: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(assets_dir(project_root))?;
    std::fs::create_dir_all(thumbnails_dir(project_root))?;
    Ok(())
}

/// 约定路径：`<app_data>/projects/{project_id}/`。
/// 用于 backfill 既有 NULL 行 + create_project 缺省时的回退。
/// project_id 用 uuid 不用 slug：可读名留给 GUI suggest_project_root 仅作展示。
pub fn convention_root(app_data_dir: &Path, project_id: &str) -> PathBuf {
    app_data_dir.join(PROJECTS_SUBDIR).join(project_id)
}
```

### 路径校验（仅对用户显式输入的 root_path）

仅在 `CreateProjectInput.root_path` 为 `Some(_)` 时校验。`None`（走约定路径）不需要校验——`convention_root` 始终产出绝对且新建路径。

| 规则 | 错误 |
|---|---|
| 必须是绝对路径 | `Validation("root_path must be absolute")` |
| 不存在 OR 是空目录 | `Validation("root_path must be empty or non-existent")` |
| 父目录已存在且可写 | `Validation("parent directory not writable: ...")` |

校验通过后：
1. `ensure_project_layout(&root_path)` 建立子目录
2. INSERT row（root_path 入库）
3. 失败回滚：若 INSERT 失败但目录已建，**不删除目录**（用户可能复用），仅记录 warn

约定路径分支（None）直接走"先生成 uuid → ensure_project_layout(convention_root(app_data, uuid)) → 单次 INSERT 带上 root_path"——无 INSERT+UPDATE 两段事务。

### 启动期统一入口（`crates/core/src/startup.rs`）

```rust
use rusqlite::{params, Connection};
use std::path::Path;

use crate::error::{CoreError, Result};
use crate::paths;

/// 在 db::open_* 之后调用，做所有跨实体的启动钩子。
/// MS1 包含：
/// 1. 回填 project.root_path NULL 行
/// 2. seed providers/models（由 spec-13 实现 seed::providers::apply）
///
/// 任一步骤失败都 panic：启动期数据不一致比运行时崩溃更难调试。
pub fn initialize(conn: &Connection, app_data_dir: &Path) -> Result<()> {
    backfill_project_roots(conn, app_data_dir)?;
    crate::seed::providers::apply(conn)?;       // spec-13 提供
    Ok(())
}

/// 给所有 root_path IS NULL 的项目分配约定路径并建目录。
/// 任一目录创建失败立即返回 Err，由 `initialize` panic（启动期硬失败）。
pub fn backfill_project_roots(conn: &Connection, app_data_dir: &Path) -> Result<()> {
    let ids: Vec<String> = {
        let mut stmt = conn.prepare("SELECT id FROM project WHERE root_path IS NULL")?;
        stmt.query_map([], |row| row.get(0))?
            .collect::<std::result::Result<_, _>>()?
    };

    for id in &ids {
        let root = paths::convention_root(app_data_dir, id);
        paths::ensure_project_layout(&root)
            .map_err(|e| CoreError::Validation(format!(
                "backfill: failed to create layout for {id} at {}: {e}",
                root.display()
            )))?;
        conn.execute(
            "UPDATE project SET root_path = ?1 WHERE id = ?2",
            params![root.to_string_lossy(), id],
        )?;
    }

    if !ids.is_empty() {
        tracing::info!("backfilled root_path for {} project(s)", ids.len());
    }
    Ok(())
}
```

> **设计说明**：硬失败方案优于"跳过坏行 + 运行时再撞"。理由：
> - 应用层把 `Project.root_path: String`（非 Option）当作不变式；NULL 行存在会让任何后续 SELECT 反序列化 panic，行为更糟
> - 启动期 panic 用户能在日志立刻看到原因；运行时 panic 通常发生在 IPC 响应或异步任务里，难以归因
> - 实际触发概率极低（仅 MS0 调试数据），用户可手动删除 `mango.db` 重来

### Tauri 启动序

`src-tauri/src/lib.rs` 修改：
```rust
.setup(move |app| {
    builder.mount_events(app);

    let app_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_dir)?;
    let db_path = app_dir.join(METADATA_DB_FILENAME);

    let db = tauri::async_runtime::block_on(mango_core::db::open_async(&db_path))
        .expect("Failed to initialize metadata database");

    // 统一启动钩子（backfill + seed）；任一步失败硬性失败启动
    let app_data_for_init = app_dir.clone();
    tauri::async_runtime::block_on(async {
        db.call(move |conn| {
            mango_core::startup::initialize(conn, &app_data_for_init)
                .map_err(|e| tokio_rusqlite::Error::Other(Box::new(e)))
        })
        .await
    })
    .expect("Failed to run startup initialize");

    app.manage(state::AppState { db });
    Ok(())
})
```

CLI 启动同样调用：`crates/cli/src/main.rs` 在 `open_sync` 后立刻 `mango_core::startup::initialize(&conn, &app_data_dir)?;`。`app_data_dir` 通过 `dirs::data_dir()` 获取，与 Tauri 的 `app_data_dir` 同位置（MEMORY.md 记录的 `<app_data>/com.mango.app/`）。

### Tauri command：目录选择器（异步实现）

`src-tauri/src/commands/dialog.rs`（新文件）：
```rust
use tauri_plugin_dialog::{DialogExt, FilePath};

#[tauri::command]
#[specta::specta]
pub async fn pick_project_directory(
    app: tauri::AppHandle,
) -> Result<Option<String>, crate::error::IpcError> {
    // 非阻塞版本：用 tokio oneshot 等待用户选择
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |result| {
        let _ = tx.send(result.map(|fp| match fp {
            FilePath::Path(p) => p.to_string_lossy().into_owned(),
            FilePath::Url(u) => u.to_string(),
        }));
    });
    rx.await.map_err(|e| crate::error::IpcError::internal(e.to_string()))
}

#[tauri::command]
#[specta::specta]
pub async fn suggest_project_root(
    app: tauri::AppHandle,
    project_name: String,
) -> Result<String, crate::error::IpcError> {
    let app_data = app.path().app_data_dir()
        .map_err(|e| crate::error::IpcError::internal(e.to_string()))?;
    let slug = slug::slugify(&project_name);
    // 注意：suggest 用 slug 仅做用户友好展示，最终入库以 uuid 为底；用户若不改建议路径会落到这里。
    let dir = if slug.is_empty() {
        app_data.join("projects").join("untitled")
    } else {
        app_data.join("projects").join(slug)
    };
    Ok(dir.to_string_lossy().into_owned())
}
```

> **设计说明**：
> - **不**用 `blocking_pick_folder`——它会阻塞 tokio 工作线程，影响其他 IPC 调用。改用回调 + `oneshot` 等待。
> - `suggest_project_root` 返回的是建议值，用户可在表单里改；最终入库的 `root_path` 由前端提交什么就是什么。如果用户接受建议值并提交，约定路径校验会通过（不存在的新目录满足"不存在 OR 是空目录"）。

### Tauri capabilities 配置

`src-tauri/capabilities/default.json` 加入对话框权限：
```json
{
  "permissions": [
    "core:default",
    "dialog:allow-open"
  ]
}
```

> 实施时按 `tauri-plugin-dialog` 当时文档核对 permission 名（v2 可能用 `dialog:default` 或 `dialog:allow-open`）。

### 前端创建对话框改造

`ProjectListPage.tsx` 创建表单字段顺序：
1. 名称（必填，`onBlur` 触发 `suggest_project_root` 更新"位置"字段建议值；用户已改过则不覆盖）
2. 描述（可选）
3. **位置**（必填）：文本框 + "选择"按钮（调用 `pick_project_directory`，选中后回填）+ 提示：`目录将自动创建 assets/ 和 thumbnails/ 子文件夹`
4. 校验提示与后端 `Validation` 错误一一映射

> **设计说明**：前端始终发送 `root_path: string`（即把建议值或用户改过的值原样提交），不发送 `null`。`null` 是 CLI 专属语义。

### CLI 改造

`crates/cli/src/commands/project.rs` 的 `Create` 命令：
```rust
#[derive(Args)]
pub struct CreateArgs {
    #[arg(long)]
    pub name: String,
    /// 缺省时自动落到 `<app_data>/projects/{uuid}/`
    #[arg(long)]
    pub root_path: Option<PathBuf>,
    #[arg(long)]
    pub description: Option<String>,
}
```

逻辑：直接把 `CreateProjectInput { root_path: args.root_path.map(...), ... }` 透传给 `core::queries::project::create`。core 内部分支：
- `Some(p)` → 走 §"路径校验" → ensure_project_layout → INSERT
- `None` → 生成 uuid → ensure_project_layout(convention_root(app_data, uuid)) → 单次 INSERT 带 root_path

`core::queries::project::create` 因此需要新增参数 `app_data_dir: &Path`（仅用于 None 分支兜底）。CLI 通过 `dirs::data_dir().join("com.mango.app")` 传入；Tauri 通过 `AppState` 额外存一份 `app_data_dir`。

### 删除行为

`delete_project` MS1 实现**只删 DB 行**（保留 root_path 目录与文件）。command 层在 docstring 注明：
```rust
/// 仅删除项目元数据。文件系统中的项目目录与素材保留。
/// V2 将提供选项 `purge_files: bool` 联动清理。
```

### 错误场景

| 场景 | 行为 |
|---|---|
| 创建时显式 root_path 已存在且非空 | 拒绝；返回 Validation 错误 |
| 创建时显式 root_path 父目录不存在 | 拒绝（不递归 mkdir 父目录，避免误建在奇怪位置） |
| 创建时显式 root_path 不可写 | 拒绝；尝试在父目录建临时文件检测 |
| 创建时缺省 root_path（CLI / 内部） | 走 convention_root；目录创建失败 → 返回 CoreError::Io |
| backfill 中 mkdir 失败 | `initialize` 返回 Err → `expect` panic → 应用启动失败 |
| 启动后用户手动删除了 root_path 目录 | MS1 不主动检测；任务 7 导入时遇到失败再报错 |
| ALTER TABLE 重复执行 | 由迁移版本号机制天然防重 |

## 测试策略

### 单元测试

`crates/core/src/paths.rs`：
- `assets_dir` / `thumbnails_dir` 路径拼接
- `ensure_project_layout` 幂等（连续两次调用都成功）
- `convention_root` 拼接正确，含 `projects/` 子目录

`crates/core/src/db/queries/project.rs`：
- `create` 显式传 root_path → 行写入正确
- `create` 缺省 root_path → 自动分配到约定路径 + 目录已建（单次 INSERT，无 UPDATE）
- `create` root_path 是相对路径 → Validation 错误
- `create` root_path 已存在且非空 → Validation 错误

`crates/core/src/startup.rs`：
- `backfill_project_roots`：插入 NULL 行 → 调 backfill → 行被更新且目录已建
- `backfill_project_roots`：mkdir 失败（路径含非法字符）→ 返回 Err（不 panic 函数自身，panic 留给上层 expect）
- `initialize`：组合 backfill + seed，两者依次成功
- `initialize`：backfill 失败时不应继续 seed

### 集成测试（`crates/core/tests/`）

`tests/project_lifecycle.rs`：
- 端到端：建临时 app_data_dir → open_sync → initialize → create 项目 → 验证 assets/ thumbnails/ 存在 → delete → 验证目录仍在但 DB 行没了

测试用 `tempfile::tempdir()`，落盘但作用域退出自动清理；不能用 `:memory:`，因为要测真实文件系统操作。

## 验收标准

- [ ] `002_add_project_root_path.sql` 加入迁移，重复运行幂等
- [ ] `Project.root_path: String`（非 Option）导出到 TS bindings
- [ ] `CreateProjectInput.root_path: Option<String>` 导出到 TS bindings
- [ ] GUI 创建新项目后，文件系统上有 `<root_path>/{assets,thumbnails}/`
- [ ] GUI "位置" 字段允许选择 / 修改；不指定时由 `suggest_project_root` 提供建议
- [ ] 既有 NULL 项目（MS0 测试数据）首次启动后被 backfill 到约定路径并建目录；backfill 失败时应用启动失败并 panic 日志含原因
- [ ] `pick_project_directory` IPC 用非阻塞实现（无 `blocking_*` 调用）
- [ ] CLI `mango project create --name X` 不指定 `--root-path` 时单次 INSERT 写入 `<app_data>/projects/{uuid}/`
- [ ] CLI `mango project create --root-path /existing-non-empty` 报错
- [ ] CLI 启动也调用 `startup::initialize`（与 Tauri 共享同一钩子）
- [ ] `cargo test --workspace` 全绿；新增 paths/startup/queries 测试覆盖率不低于既有 project CRUD
- [ ] `pnpm typecheck` 全绿；ProjectListPage 创建对话框可手测建项目并看到目录生成
- [ ] `src-tauri/capabilities/default.json` 加入 dialog 权限

## 依赖关系

- **前置**：SPEC-03（迁移框架）、SPEC-04（project CRUD）、SPEC-05（Tauri command 层）、SPEC-09（项目页面 UI 框架）
- **平行**：spec-13 的 `seed::providers::apply` 由 `startup::initialize` 调用；两 spec 实现互相 stub 即可解耦提交
- **后续**：
  - SPEC-12（asset import pipeline）依赖 `paths::assets_dir` / `thumbnails_dir`
  - SPEC-14（主体详情页参考图预览）依赖项目 root 来解析相对路径
  - 全部 MS1 任务（5/6/7）必须在本 spec 完成后方能开工

## 与原任务文档的差异

`开发任务.md` 第 7 个任务"素材导入管线"假设"项目 assets 目录"已存在，但 MS0 实际未实现该概念。本 spec 补齐这块缺口；建议：
- `开发任务.md` 在任务清单顶部插入"任务 0：项目数据目录"指向本 spec
- 或将本 spec 提到 spec-04（project CRUD）的迭代修订中——但因属 MS1 范畴决策（MS0 已合并），放在 MS1 目录更合适
