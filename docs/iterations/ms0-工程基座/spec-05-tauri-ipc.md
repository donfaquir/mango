# SPEC-05: Tauri IPC 层

## 概述

通过 `#[tauri::command]` 将 core crate 的 Project CRUD 暴露为 IPC 接口，使用 tauri-specta 自动生成类型安全的 TypeScript 绑定。建立 Tauri 状态管理模式和统一错误处理。

### 数据库分层（本 spec 边界）

`技术选型.md` §5.8 规定每个项目独立 DB（`~/DVT-Projects/{project_id}/project.db`），整体采用**全局元数据 DB + 项目级业务 DB**的两层结构：

| 层 | 文件 | 持有表 | 何时打开 |
|----|------|--------|---------|
| 全局元数据 | `{app_data_dir}/mango.db` | `project`（仅项目列表元信息） | 应用启动时一次 |
| 项目级业务 | `{project_dir}/project.db` | `episode` / `shot` / `character_profile` / `asset` / ... | 用户"打开项目"时按需 |

**本 spec 仅落地全局元数据 DB**，托管在 `AppState.db` 中，用于 Project 实体的 CRUD。项目级 DB 的开启/切换/缓存策略由后续 spec（MS1 阶段）定义，不属本 spec 范围。

> ⚠️ 当前 `001_initial.sql` 暂时将全部 17 张业务表也建在了同一个 schema 里。MS1 引入项目级 DB 时需拆分迁移文件：`001_metadata.sql`（仅 project + _migrations）+ `002_project.sql`（其余 16 张业务表 + 关联表）。本 spec 不做这次拆分，但实现者应知晓此约束。

## 技术方案

### 目录/文件结构

```
src-tauri/src/
├── main.rs                 # 入口（调用 lib::run）
├── lib.rs                  # Tauri Builder 配置 + specta 注册
├── state.rs                # AppState 定义
├── error.rs                # IPC 错误类型
└── commands/
    ├── mod.rs              # commands 模块导出
    └── project.rs          # Project 相关 commands
```

### 依赖项

**src-tauri/Cargo.toml** 新增依赖：
```toml
[dependencies]
tauri-specta = { version = "2.0.0-rc.25", features = ["typescript", "derive"] }
specta = { version = "2.0.0-rc.25" }
specta-typescript = "0.0.12"
tokio-rusqlite = { workspace = true }
```

**crates/core/Cargo.toml** 新增依赖（用于 IPC 类型反射，见下文「类型生成职责划分」一节）：
```toml
[dependencies]
specta = { version = "2.0.0-rc.25", features = ["chrono", "uuid"] }
```

**workspace `Cargo.toml`** 新增公共版本（保证 core / src-tauri specta 版本一致）：
```toml
[workspace.dependencies]
specta = { version = "2.0.0-rc.25" }
```

> **版本说明**：tauri-specta 2.x 目前仍为 RC 状态（截至 2026-05-21），是 Tauri 2 唯一可用的版本。无稳定替代方案。core 引入 specta 不破坏「core 不依赖 Tauri」的边界——specta 是独立的类型反射库，与 ts-rs 同级。

### 核心实现

#### 应用状态（state.rs）

```rust
use tokio_rusqlite::Connection as AsyncConnection;

/// Tauri 全局托管状态
pub struct AppState {
    pub db: AsyncConnection,
}
```

> **设计说明**：
> - `tokio_rusqlite::Connection` 内部是 `Arc` + channel 架构（请求发送到独立 DB 线程），本身即为 `Clone + Send + Sync`，无需外层 `Arc<Mutex<>>`。
> - 多个 `call()` 可并发发起，由内部队列串行执行，不会阻塞 tokio runtime。

#### IPC 错误处理（error.rs）

```rust
use serde::Serialize;
use specta::Type;

/// IPC 错误封装 — 必须实现 Serialize 才能跨 IPC 传输
#[derive(Debug, Serialize, Type)]
pub struct IpcError {
    pub message: String,
    pub code: String,
}

impl From<mango_core::error::CoreError> for IpcError {
    fn from(err: mango_core::error::CoreError) -> Self {
        match &err {
            mango_core::error::CoreError::NotFound { entity, id } => IpcError {
                message: format!("{} '{}' not found", entity, id),
                code: "NOT_FOUND".into(),
            },
            mango_core::error::CoreError::Validation(msg) => IpcError {
                message: msg.clone(),
                code: "VALIDATION_ERROR".into(),
            },
            mango_core::error::CoreError::Sqlite(e) => IpcError {
                message: format!("Database error: {}", e),
                code: "DB_ERROR".into(),
            },
        }
    }
}

impl From<tokio_rusqlite::Error> for IpcError {
    fn from(err: tokio_rusqlite::Error) -> Self {
        IpcError {
            message: format!("Database connection error: {}", err),
            code: "DB_ERROR".into(),
        }
    }
}

impl From<IpcError> for tauri::ipc::InvokeError {
    fn from(err: IpcError) -> Self {
        tauri::ipc::InvokeError::from(serde_json::to_value(err).unwrap())
    }
}
```

> **设计说明**：
> - 实现 `From<CoreError>` 和 `From<tokio_rusqlite::Error>` 两个转换，分别处理业务错误和连接层错误。
> - `CoreError` 只有 `Sqlite` / `NotFound` / `Validation` 三个变体（SPEC-04 定义），此处逐一匹配。
> - `tokio_rusqlite::Error` 在 `call()` 外层返回（如连接已关闭），单独处理。

#### 辅助函数：消除 command 样板代码（commands/mod.rs）

```rust
pub mod project;

use crate::error::IpcError;
use crate::state::AppState;
use tauri::State;

/// 执行同步 DB 操作的统一辅助。
///
/// 闭包在 tokio-rusqlite 的独立 DB 线程上执行，签名为
/// `FnOnce(&mut rusqlite::Connection) -> Result<T, CoreError>`。
pub async fn with_db<T, F>(state: &State<'_, AppState>, f: F) -> Result<T, IpcError>
where
    T: Send + 'static,
    F: FnOnce(&mut rusqlite::Connection) -> mango_core::error::Result<T> + Send + 'static,
{
    // tokio_rusqlite::Connection::call 要求闭包返回 Result<R, tokio_rusqlite::Error>，
    // 没有 Other(Box<dyn Error>) 类型的兜底变体，因此不能直接把 CoreError 映射进去。
    // 用 Ok(f(conn)) 把整个 mango_core::Result<T> 作为成功值原样穿透，
    // 在外层再拆开域错误。这样得到清晰的两层错误：
    //   外层 tokio_rusqlite::Error  → 连接 / 线程 panic 等基础设施错误
    //   内层 CoreError              → 业务 / SQL 语义错误
    let domain_result: mango_core::error::Result<T> = state
        .db
        .call(move |conn| Ok(f(conn)))
        .await
        .map_err(IpcError::from)?; // tokio_rusqlite::Error → IpcError

    domain_result.map_err(IpcError::from) // CoreError → IpcError
}
```

> **设计说明**：
> - tokio-rusqlite 0.7 的 `Connection::call` 实际签名为
>   `async fn call<F, R>(&self, f: F) -> Result<R, tokio_rusqlite::Error>`
>   其中 `F: FnOnce(&mut rusqlite::Connection) -> Result<R, tokio_rusqlite::Error>`。
>   **闭包参数是 `&mut Connection` 不是 `&Connection`**；项目 query 函数接受 `&Connection`，靠 Rust 的自动 reborrow 兼容。
> - 闭包错误类型固定为 `tokio_rusqlite::Error`，且该枚举无 `Other(...)` 这种通用包装变体（实测代码见 `crates/core/src/db/connection.rs:42` 与 `:52`：只用了 `ConnectionClosed` / `Close` / `Error(rusqlite::Error)` 三个变体）。
> - 因此把 `CoreError` 强行映射成 `tokio_rusqlite::Error` 是不通的——本实现采用「闭包内永远返回 `Ok(domain_result)`，外层先拆基础设施错误再拆域错误」的模式。这是 tokio-rusqlite 文档示例同款写法。
> - `with_db` 封装后，command 函数只需关注业务逻辑，无需重复这套包装。

#### Project Commands（commands/project.rs）

```rust
use tauri::State;
use mango_core::models::project::{
    CreateProjectInput, ListProjectsOptions, Project, UpdateProjectInput,
};
use mango_core::db::queries::project as project_queries;
use crate::state::AppState;
use crate::error::IpcError;
use super::with_db;

#[tauri::command]
#[specta::specta]
pub async fn create_project(
    state: State<'_, AppState>,
    input: CreateProjectInput,
) -> Result<Project, IpcError> {
    with_db(&state, move |conn| project_queries::create(conn, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_project(
    state: State<'_, AppState>,
    id: String,
) -> Result<Project, IpcError> {
    with_db(&state, move |conn| project_queries::get_by_id(conn, &id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_projects(
    state: State<'_, AppState>,
    opts: Option<ListProjectsOptions>,
) -> Result<Vec<Project>, IpcError> {
    let opts = opts.unwrap_or_default();
    with_db(&state, move |conn| project_queries::list(conn, opts)).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_project(
    state: State<'_, AppState>,
    id: String,
    input: UpdateProjectInput,
) -> Result<Project, IpcError> {
    with_db(&state, move |conn| project_queries::update(conn, &id, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_project(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| project_queries::delete(conn, &id)).await
}
```

> **设计说明**：
> - 每个 command 仅一行业务逻辑，无样板代码。
> - `list_projects` 接受 `Option<ListProjectsOptions>`，前端不传时使用默认分页。
> - 所有参数通过 `move` 捕获进闭包（闭包需要 `Send + 'static`），避免生命周期问题。

#### Tauri Builder + Specta 集成（lib.rs）

```rust
use tauri::Manager;
use tauri_specta::{collect_commands, Builder};
use specta_typescript::Typescript;

mod commands;
mod error;
mod state;

/// 全局元数据库文件名（项目级业务 DB 由后续 spec 引入，不在此处）。
const METADATA_DB_FILENAME: &str = "mango.db";

pub fn run() {
    // 1. 创建 specta builder，收集所有 commands。
    //    Builder 的所有权要点：
    //      - .commands(...)         消费 self 链式返回
    //      - .export(&self, ...)    借用，可重复调用
    //      - .invoke_handler(&self) 借用，返回独立 owned 闭包
    //      - .mount_events(&self)   借用
    //    因此可以先 export、再 invoke_handler，再把 builder 整体 move 进 setup 闭包
    //    供 mount_events 使用——只要这三步全用借用语义，顺序合法。
    let builder = Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::project::create_project,
            commands::project::get_project,
            commands::project::list_projects,
            commands::project::update_project,
            commands::project::delete_project,
        ]);

    // 2. 开发模式下自动生成 TypeScript 绑定（仅 debug build）。
    #[cfg(debug_assertions)]
    builder
        .export(Typescript::default(), "../src/lib/bindings/commands.ts")
        .expect("Failed to export typescript bindings");

    // 3. 构建 Tauri 应用。
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            // ── 同步初始化全局元数据 DB ──
            // 必须在 setup 内同步完成，确保 AppState 在任何 command 被调用前已注册。
            //
            // 为什么这里 block_on 是安全的：Tauri 2 的 setup 闭包在 wry 主循环启动之前、
            // 在 tauri::async_runtime 的执行栈之外被调用，没有当前 task 阻塞自身的风险。
            // 如未来 Tauri 调整 setup 调度模型，需替换为 std::thread + Runtime::new().block_on()
            // 隔离 runtime 的写法。
            let app_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_dir)?;
            let db_path = app_dir.join(METADATA_DB_FILENAME);

            let db = tauri::async_runtime::block_on(mango_core::db::open_async(&db_path))
                .expect("Failed to initialize metadata database");

            app.manage(state::AppState { db });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

> **设计说明**：
> - **Builder 所有权流**：上面注释中描述的三步借用顺序经 tauri-specta 2.0.0-rc.25 API 验证可行；若升级 RC 版本，需重新核对方法签名是否仍接受 `&self`。
> - **同步初始化**：用 `block_on` 在 `setup` 中完成 DB 初始化，确保 `AppState` 在任何 command 被调用前注册完毕。避免异步 spawn 导致的竞态（command 在 state 注册前被调用会 panic）。
> - `open_async` 返回 `Result<AsyncConnection, DbError>`，此处用 `.expect()` 处理——DB 初始化失败是不可恢复的致命错误，直接崩溃并给出明确错误信息。
> - `#[cfg(debug_assertions)]` 确保 TypeScript 绑定仅在开发模式生成，release build 不含此逻辑。
> - **DB 文件名 const 化**：`METADATA_DB_FILENAME` 抽常量便于测试时替换为 `:memory:` 或临时路径。

### 类型生成职责划分（ts-rs vs tauri-specta）

#### 问题

仓库现状（SPEC-04 落地）：`crates/core/src/models/project.rs` 上 `Project` / `CreateProjectInput` / `UpdateProjectInput` / `ListProjectsOptions` 都已经 `#[derive(TS)] #[ts(export)]`，ts-rs 已经在 `src/lib/bindings/types/` 下生成了四个 `.ts` 文件。

但 tauri-specta 在生成 `commands.ts` 时会**默认 inline 内联生成**所有 command 参数 / 返回值的类型定义。如果不做区分，前端会看到两份相同名称的 `Project` 类型——一份在 `bindings/types/Project.ts`，一份在 `bindings/commands.ts` 内联——容易误用且 IDE 跳转混乱。

#### 决策：tauri-specta 拥有所有 command 链路上的类型

| 类型用途 | 反射库 | 输出位置 | 例 |
|---------|--------|---------|----|
| Command 参数 / 返回值 | **specta**（由 tauri-specta 收集） | `src/lib/bindings/commands.ts`（内联） | `Project`, `CreateProjectInput` |
| Tauri Event payload | ts-rs | `src/lib/bindings/types/` | （MS2 引入，如 `TaskStatusChangedPayload`） |
| 独立纯数据 enum / 常量 | ts-rs | `src/lib/bindings/types/` | （暂无） |

理由：
- specta 与 tauri-specta 同源，自动跟随 command 签名变化，零手工同步。
- ts-rs 在 derive 时即写盘，独立于 Tauri runtime，更适合非 IPC 场景（event payload 在前端通过 `listen()` 接收，不走 specta command 通道）。
- 双轨而非二选一：未来 MS2 引入 event 时不需要再换工具，且 core 仍可独立于 src-tauri 单独输出 TS 类型给 CLI 文档等用途。

#### 实施变更

**`crates/core/src/models/project.rs`** —— 移除 `ts_rs::TS` derive，换用 `specta::Type`：

```rust
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: String,
    pub style_prompt: String,
    pub global_seed: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Type)]
pub struct CreateProjectInput {
    pub name: String,
    #[serde(default)] pub description: Option<String>,
    #[serde(default)] pub style_prompt: Option<String>,
    #[serde(default)] pub global_seed: Option<i64>,
}

// UpdateProjectInput / ListProjectsOptions 同理替换 derive。
```

**清理动作**：
- 删除 `src/lib/bindings/types/Project.ts`、`CreateProjectInput.ts`、`UpdateProjectInput.ts`、`ListProjectsOptions.ts`
- `src/lib/bindings/types/` 目录保留（MS2 起会被 ts-rs 重新填充 event payload）

**前端引用规则**：
- IPC 调用 → `import { createProject, type Project } from "@/lib/bindings/commands"`
- Event 监听 → `import type { XxxPayload } from "@/lib/bindings/types/XxxPayload"`（MS2 后才有）

> 备选方案（**未采用**）：保留 ts-rs 双写，由人工保证两边定义一致。被否决因为 IPC 类型应有单一源，避免出现「IPC 调用通过、但 IDE 自动 import 错文件导致类型不匹配」的隐蔽 bug。

### 配置

**src-tauri/capabilities/default.json**（无需额外权限，保持现有配置）：
```json
{
  "identifier": "default",
  "description": "Default capabilities",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "shell:allow-open",
    "core:event:default"
  ]
}
```

## 接口定义

**Rust IPC Commands：**
```rust
async fn create_project(state, input: CreateProjectInput) -> Result<Project, IpcError>
async fn get_project(state, id: String) -> Result<Project, IpcError>
async fn list_projects(state, opts: Option<ListProjectsOptions>) -> Result<Vec<Project>, IpcError>
async fn update_project(state, id: String, input: UpdateProjectInput) -> Result<Project, IpcError>
async fn delete_project(state, id: String) -> Result<(), IpcError>
```

**生成的 TypeScript 绑定（src/lib/bindings/commands.ts）：**
```typescript
// 自动生成 — 不要手动修改
import { invoke } from "@tauri-apps/api/core";

export async function createProject(input: CreateProjectInput): Promise<Project> { ... }
export async function getProject(id: string): Promise<Project> { ... }
export async function listProjects(opts?: ListProjectsOptions): Promise<Project[]> { ... }
export async function updateProject(id: string, input: UpdateProjectInput): Promise<Project> { ... }
export async function deleteProject(id: string): Promise<void> { ... }
```

## 验收标准

```bash
# 1. 编译通过
cargo build -p mango-tauri

# 2. 开发模式启动（同时验证 specta 生成）
pnpm tauri dev
# → 检查 src/lib/bindings/commands.ts 已生成
# → 检查文件中包含 createProject / listProjects 等函数

# 3. 前端调用测试（在浏览器 console 或 React 组件中）
# import { createProject, listProjects } from '@/lib/bindings/commands';
# const project = await createProject({ name: "test" });
# const list = await listProjects();
# console.log(list); // 应包含刚创建的项目

# 4. 错误传递验证
# await createProject({ name: "" });
# → 应返回 { message: "name cannot be empty", code: "VALIDATION_ERROR" }

# 5. TypeScript 编译无错误
pnpm typecheck
```

## 依赖关系

- **前置**：
  - SPEC-01（src-tauri crate 结构）
  - SPEC-03（DB 连接 `open_async`）
  - SPEC-04（Project CRUD 函数 + `CoreError` 定义）
- **后续**：
  - SPEC-07（类型生成管线）扩展此配置
  - SPEC-08（App Shell UI）通过生成的绑定调用后端
  - SPEC-09（项目管理页面）直接使用这些 commands

## 与原版本的变更摘要

### 第一轮重写（修正编译错误与架构不一致）

| 变更项 | 旧方案 | 新方案 | 原因 |
|--------|--------|--------|------|
| AppState.db 类型 | `Arc<Mutex<AsyncConnection>>` | `tokio_rusqlite::Connection`（直接存储） | tokio_rusqlite 内部已是 Arc+channel 架构，外层 Mutex 多余且会串行化所有调用 |
| DB 初始化时机 | `setup` 中 `async_runtime::spawn` | `setup` 中 `async_runtime::block_on` | 避免 state 注册与 command 调用之间的竞态（command 可能在 state 注册前被调用导致 panic） |
| IpcError From 实现 | 匹配 `CoreError::Database`（不存在的变体） | 匹配 `CoreError::Sqlite` + 单独实现 `From<tokio_rusqlite::Error>` | CoreError 实际只有 Sqlite/NotFound/Validation 三个变体 |
| list_projects 参数 | 无参数（编译不过） | `opts: Option<ListProjectsOptions>` | core 层 `list()` 需要 `ListProjectsOptions` 参数 |
| call() 返回值处理 | 单层 map_err（类型不匹配） | 两层展开：外层 tokio_rusqlite::Error，内层 CoreError | `call()` 返回 `Result<R, Error>`，当 R 本身是 Result 时需要两层处理 |
| command 样板代码 | 每个 command 重复 db.lock + call + map_err | 提取 `with_db` 辅助函数 | 减少重复，新增 command 时只需一行 |
| tauri-specta 版本 | `2.0.0-rc`（模糊） | `2.0.0-rc.25`（精确锁定） | 明确版本避免构建不可复现 |
| specta-typescript 版本 | `0.0.8` | `0.0.12` | 使用最新版本 |
| AppState::new | 返回 `Result<Self, CoreError>`（类型不匹配） | 移除 new 方法，setup 中直接构造 | open_async 返回 DbError 非 CoreError，且初始化失败应直接 panic |

### 第二轮重写（评审反馈，修复 4 个落地阻塞问题）

| # | 变更项 | 旧方案 | 新方案 | 原因 |
|---|--------|--------|--------|------|
| 1 | `with_db` 闭包签名 | `F: FnOnce(&Connection) -> mango_core::Result<T>` + 单调用 `state.db.call(f)` | `F: FnOnce(&mut Connection) -> mango_core::Result<T>` + 闭包内 `Ok(f(conn))` 把域 Result 原样穿透，外层两次 `map_err` | tokio-rusqlite 0.7 的 `call` 要求闭包返回 `Result<R, tokio_rusqlite::Error>` 且参数为 `&mut Connection`，且 `tokio_rusqlite::Error` 无 `Other(...)` 通用包装变体，CoreError 无法直接映射进去 |
| 2 | Builder 所有权流 | 隐式：依次 `export` → `invoke_handler` → `move` 进 setup，未注明合法性 | 显式在代码注释中标注三步均为 `&self` 借用，并说明跨 RC 版本升级需重核 | 让审查者无需翻 tauri-specta 源码即可判断顺序合法 |
| 3 | 类型生成职责 | 未规定——ts-rs 已 export Project 类型，tauri-specta 又会内联生成同名类型 | 新增「类型生成职责划分」一节：command 链路类型由 specta 拥有（移除 `#[ts(export)]`），ts-rs 仅留给 event payload 等非 command 场景 | 避免前端出现两份 `Project` 定义；建立 IPC 类型单一来源 |
| 4 | mango.db 范围 | 未澄清——读者可能以为所有业务表都在此 DB | 新增「数据库分层」段：本 spec 的 `mango.db` 仅承载 `project` 元数据表，项目级 DB 由 MS1 引入；标注 `001_initial.sql` 后续需拆分 | 与 `技术选型.md` §5.8 的「每项目一个 DB」对齐，避免实现者将业务表错误地塞进全局 DB |
| - | DB 路径字面量 | 内联 `"mango.db"` | 抽常量 `METADATA_DB_FILENAME` | 测试时便于替换为 `:memory:` 或临时路径 |
| - | core crate 依赖 | 仅 ts-rs | 新增 `specta` | 让 core 类型可被 tauri-specta 反射；specta 是纯类型库，不破坏「core 不依赖 Tauri」边界 |
