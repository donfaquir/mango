# SPEC-05: Tauri IPC 层

## 概述

通过 `#[tauri::command]` 将 core crate 的 Project CRUD 暴露为 IPC 接口，使用 tauri-specta 自动生成类型安全的 TypeScript 绑定。建立 Tauri 状态管理模式和统一错误处理。

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

src-tauri/Cargo.toml：
```toml
[dependencies]
tauri = { workspace = true }
tauri-specta = { version = "2.0.0-rc", features = ["typescript", "derive"] }
specta = { version = "2.0.0-rc", features = ["typescript"] }
specta-typescript = "0.0.8"
mango-core = { path = "../crates/core" }
tokio = { workspace = true }
tokio-rusqlite = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }

[build-dependencies]
tauri-build = { version = "2", features = [] }
```

### 核心实现

#### 应用状态（state.rs）

```rust
use tokio_rusqlite::Connection as AsyncConnection;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Tauri 全局托管状态
pub struct AppState {
    pub db: Arc<Mutex<AsyncConnection>>,
}

impl AppState {
    pub async fn new(db_path: &std::path::Path) -> Result<Self, mango_core::error::CoreError> {
        let conn = mango_core::db::open_async(db_path).await
            .map_err(|e| mango_core::error::CoreError::Database(
                rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(1), Some(e.to_string())
                )
            ))?;
        Ok(Self {
            db: Arc::new(Mutex::new(conn)),
        })
    }
}
```

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
            mango_core::error::CoreError::Database(e) => IpcError {
                message: format!("Database error: {}", e),
                code: "DB_ERROR".into(),
            },
        }
    }
}

// 让 tauri::command 可以返回 Result<T, IpcError>
impl From<IpcError> for tauri::ipc::InvokeError {
    fn from(err: IpcError) -> Self {
        tauri::ipc::InvokeError::from(serde_json::to_value(err).unwrap())
    }
}
```

#### Project Commands（commands/project.rs）

```rust
use tauri::State;
use mango_core::models::project::{Project, CreateProjectInput, UpdateProjectInput};
use mango_core::db::queries::project as project_queries;
use crate::state::AppState;
use crate::error::IpcError;

#[tauri::command]
#[specta::specta]
pub async fn create_project(
    state: State<'_, AppState>,
    input: CreateProjectInput,
) -> Result<Project, IpcError> {
    let db = state.db.lock().await;
    let result = db.call(move |conn| {
        project_queries::create(conn, input).map_err(|e| e.into())
    }).await.map_err(|e| IpcError {
        message: e.to_string(),
        code: "DB_ERROR".into(),
    })?;
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn get_project(
    state: State<'_, AppState>,
    id: String,
) -> Result<Project, IpcError> {
    let db = state.db.lock().await;
    let result = db.call(move |conn| {
        project_queries::get_by_id(conn, &id).map_err(|e| e.into())
    }).await.map_err(|e| IpcError {
        message: e.to_string(),
        code: "DB_ERROR".into(),
    })?;
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn list_projects(
    state: State<'_, AppState>,
) -> Result<Vec<Project>, IpcError> {
    let db = state.db.lock().await;
    let result = db.call(|conn| {
        project_queries::list(conn).map_err(|e| e.into())
    }).await.map_err(|e| IpcError {
        message: e.to_string(),
        code: "DB_ERROR".into(),
    })?;
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn update_project(
    state: State<'_, AppState>,
    id: String,
    input: UpdateProjectInput,
) -> Result<Project, IpcError> {
    let db = state.db.lock().await;
    let result = db.call(move |conn| {
        project_queries::update(conn, &id, input).map_err(|e| e.into())
    }).await.map_err(|e| IpcError {
        message: e.to_string(),
        code: "DB_ERROR".into(),
    })?;
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_project(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), IpcError> {
    let db = state.db.lock().await;
    db.call(move |conn| {
        project_queries::delete(conn, &id).map_err(|e| e.into())
    }).await.map_err(|e| IpcError {
        message: e.to_string(),
        code: "DB_ERROR".into(),
    })?;
    Ok(())
}
```

#### Tauri Builder + Specta 集成（lib.rs）

```rust
use tauri_specta::{collect_commands, Builder};
use specta_typescript::Typescript;

mod commands;
mod error;
mod state;

pub fn run() {
    // 1. 创建 specta builder，收集所有 commands
    let builder = Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::project::create_project,
            commands::project::get_project,
            commands::project::list_projects,
            commands::project::update_project,
            commands::project::delete_project,
        ]);

    // 2. 开发模式下自动生成 TypeScript 绑定
    #[cfg(debug_assertions)]
    builder
        .export(Typescript::default(), "../src/lib/bindings/commands.ts")
        .expect("Failed to export typescript bindings");

    // 3. 构建 Tauri 应用
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            // 初始化数据库连接
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let app_dir = app_handle.path().app_data_dir().unwrap();
                std::fs::create_dir_all(&app_dir).unwrap();
                let db_path = app_dir.join("mango.db");

                let state = state::AppState::new(&db_path).await
                    .expect("Failed to initialize database");
                app_handle.manage(state);
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 配置

**src-tauri/capabilities/default.json** 更新（如需要文件系统访问）：
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
async fn list_projects(state) -> Result<Vec<Project>, IpcError>
async fn update_project(state, id: String, input: UpdateProjectInput) -> Result<Project, IpcError>
async fn delete_project(state, id: String) -> Result<(), IpcError>
```

**生成的 TypeScript 绑定（src/lib/bindings/commands.ts）：**
```typescript
// 自动生成 — 不要手动修改
import { invoke } from "@tauri-apps/api/core";

export async function createProject(input: CreateProjectInput): Promise<Project> { ... }
export async function getProject(id: string): Promise<Project> { ... }
export async function listProjects(): Promise<Project[]> { ... }
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
# → 应返回 { message: "...", code: "VALIDATION_ERROR" }

# 5. TypeScript 编译无错误
npx tsc --noEmit
```

## 依赖关系

- **前置**：
  - SPEC-01（src-tauri crate 结构）
  - SPEC-03（DB 连接 open_async）
  - SPEC-04（Project CRUD 函数）
- **后续**：
  - SPEC-07（类型生成管线）扩展此配置
  - SPEC-08（App Shell UI）通过生成的绑定调用后端
  - SPEC-09（项目管理页面）直接使用这些 commands
