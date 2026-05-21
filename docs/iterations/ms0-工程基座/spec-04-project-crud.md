# SPEC-04: Project 表 CRUD 实现

## 概述

在 core crate 中实现 Project 实体的完整 CRUD 操作，包含数据模型定义、查询函数、错误处理和单元测试。这是第一个业务实体实现，将建立后续所有实体 CRUD 的代码模式。

## 技术方案

### 目录/文件结构

```
crates/core/src/
├── lib.rs                      # 添加 pub mod models; pub mod error;
├── error.rs                    # 统一错误类型
├── models/
│   ├── mod.rs                  # 模型导出
│   └── project.rs              # Project 结构体
└── db/
    ├── mod.rs                  # 添加 pub mod queries;
    └── queries/
        ├── mod.rs              # 查询模块导出
        └── project.rs          # Project CRUD SQL 操作
```

### 依赖项

workspace 根 `Cargo.toml` 添加：
```toml
[workspace.dependencies]
ts-rs = { version = "12", features = ["chrono-impl", "uuid-impl"] }
```

项目根 `.cargo/config.toml` 添加：
```toml
[env]
TS_RS_EXPORT_DIR = { value = "src/lib/bindings/types", relative = true }
```

core crate 的 `Cargo.toml`（workspace 引用）：
```toml
[dependencies]
uuid = { workspace = true }
chrono = { workspace = true }
ts-rs = { workspace = true }
```

dev-dependencies（已在 SPEC-03 配置）：
```toml
[dev-dependencies]
tempfile = "3"
tokio = { workspace = true, features = ["test-util", "macros"] }
```

### 核心实现

#### 错误类型（error.rs）

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("not found: {entity} with id '{id}'")]
    NotFound { entity: &'static str, id: String },

    #[error("validation error: {0}")]
    Validation(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;
```

> **设计说明**：
> - `CoreError` 是业务层错误类型，供 CRUD 查询函数使用。只包含 `rusqlite::Error`（查询函数直接操作 `&Connection`）。
> - `db::DbError` 保留在连接初始化层（含 Migration 和 AsyncSqlite 变体），不向业务层传播。Tauri command 层负责将两者统一为 `Result<T, String>`。
> - 不设 `Database(#[from] DbError)` 变体，避免与 `Sqlite` 的 `#[from]` 冲突。
> - 不设 `AsyncSqlite` 变体，避免 `tokio_rusqlite::Error<CoreError>` 递归类型问题。异步调用在 Tauri command 层手动映射。

#### 数据模型（models/project.rs）

```rust
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: String,
    pub style_prompt: String,
    pub global_seed: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct CreateProjectInput {
    pub name: String,
    pub description: Option<String>,
    pub style_prompt: Option<String>,
    pub global_seed: Option<i64>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct UpdateProjectInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub style_prompt: Option<String>,
    /// None = 不修改，Some(None) = 清空为 NULL，Some(Some(v)) = 设为 v
    pub global_seed: Option<Option<i64>>,
}

/// 列表查询参数
#[derive(Debug, Default, Deserialize, TS)]
#[ts(export)]
pub struct ListProjectsOptions {
    /// 返回条数上限，默认 50，最大 200
    pub limit: Option<i64>,
    /// 偏移量，默认 0
    pub offset: Option<i64>,
}
```

> **设计说明**：
> - `created_at`/`updated_at` 保持 `String` 类型（ISO 8601），因为前端消费时本就是字符串，且避免 rusqlite/chrono 序列化歧义。时间值完全由 DB `DEFAULT (datetime('now'))` 生成，Rust 侧不自行格式化，保证格式一致性。
> - `ts-rs` 的 `#[ts(export)]` 不指定 `export_to` 路径，输出目录通过 `.cargo/config.toml` 中的 `TS_RS_EXPORT_DIR` 持久化配置，避免硬编码相对路径。
> - `ListProjectsOptions` 为 list 接口预留分页能力。

#### 查询实现（db/queries/project.rs）

```rust
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::models::project::{
    CreateProjectInput, ListProjectsOptions, Project, UpdateProjectInput,
};

/// 创建项目（时间戳由 DB DEFAULT 生成）
pub fn create(conn: &Connection, input: CreateProjectInput) -> Result<Project> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(CoreError::Validation("name cannot be empty".into()));
    }

    let id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO project (id, name, description, style_prompt, global_seed)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            id,
            name,
            input.description.unwrap_or_default(),
            input.style_prompt.unwrap_or_default(),
            input.global_seed,
        ],
    )?;

    get_by_id(conn, &id)
}

/// 根据 ID 获取项目
pub fn get_by_id(conn: &Connection, id: &str) -> Result<Project> {
    conn.query_row(
        "SELECT id, name, description, style_prompt, global_seed, created_at, updated_at
         FROM project WHERE id = ?1",
        params![id],
        |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                style_prompt: row.get(3)?,
                global_seed: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound {
            entity: "project",
            id: id.to_string(),
        },
        other => CoreError::Sqlite(other),
    })
}

/// 获取项目列表（按创建时间倒序，支持分页）
pub fn list(conn: &Connection, opts: ListProjectsOptions) -> Result<Vec<Project>> {
    let limit = opts.limit.unwrap_or(50).min(200).max(1);
    let offset = opts.offset.unwrap_or(0).max(0);

    let mut stmt = conn.prepare(
        "SELECT id, name, description, style_prompt, global_seed, created_at, updated_at
         FROM project ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
    )?;

    let projects = stmt
        .query_map(params![limit, offset], |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                style_prompt: row.get(3)?,
                global_seed: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(projects)
}

/// 更新项目（Partial Update：仅更新传入的字段）
pub fn update(conn: &Connection, id: &str, input: UpdateProjectInput) -> Result<Project> {
    let _ = get_by_id(conn, id)?;

    if let Some(ref name) = input.name {
        if name.trim().is_empty() {
            return Err(CoreError::Validation("name cannot be empty".into()));
        }
    }

    // 动态构建 SET 子句，只更新非 None 字段
    let mut sets: Vec<&str> = vec!["updated_at = datetime('now')"];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

    if let Some(ref name) = input.name {
        sets.push("name = ?");
        params.push(Box::new(name.trim().to_string()));
    }
    if let Some(ref desc) = input.description {
        sets.push("description = ?");
        params.push(Box::new(desc.clone()));
    }
    if let Some(ref sp) = input.style_prompt {
        sets.push("style_prompt = ?");
        params.push(Box::new(sp.clone()));
    }
    if let Some(maybe_seed) = input.global_seed {
        sets.push("global_seed = ?");
        params.push(Box::new(maybe_seed)); // None → SQL NULL, Some(v) → v
    }

    // 修正占位符编号
    let mut sql = String::from("UPDATE project SET ");
    let mut idx = 1u32;
    let numbered_sets: Vec<String> = sets
        .iter()
        .map(|s| {
            if s.contains('?') {
                let result = s.replace('?', &format!("?{idx}"));
                idx += 1;
                result
            } else {
                s.to_string()
            }
        })
        .collect();
    sql.push_str(&numbered_sets.join(", "));
    sql.push_str(&format!(" WHERE id = ?{idx}"));
    params.push(Box::new(id.to_string()));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    conn.execute(&sql, param_refs.as_slice())?;

    get_by_id(conn, id)
}

/// 删除项目
pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    let affected = conn.execute("DELETE FROM project WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(CoreError::NotFound {
            entity: "project",
            id: id.to_string(),
        });
    }
    Ok(())
}
```

> **设计说明**：
> - **时间一致性**：`create` 不传 `created_at`/`updated_at`，完全依赖 DB DEFAULT；`update` 使用 `datetime('now')` SQL 表达式而非 Rust 侧格式化，消除格式差异风险。
> - **Partial Update**：使用动态 SQL 构建，彻底消除 COALESCE hack。只有明确传入的字段才会被更新，语义清晰。
> - **分页**：`list` 接受 `ListProjectsOptions`，默认 limit=50，上限 200。

## 接口定义

```rust
// 公开 API（crates/core/src/db/queries/project.rs）

pub fn create(conn: &Connection, input: CreateProjectInput) -> Result<Project>;
pub fn get_by_id(conn: &Connection, id: &str) -> Result<Project>;
pub fn list(conn: &Connection, opts: ListProjectsOptions) -> Result<Vec<Project>>;
pub fn update(conn: &Connection, id: &str, input: UpdateProjectInput) -> Result<Project>;
pub fn delete(conn: &Connection, id: &str) -> Result<()>;
```

**TypeScript 类型（自动生成）：**
```typescript
// 输出到 src/lib/bindings/types/（由 .cargo/config.toml 配置）

export interface Project {
  id: string;
  name: string;
  description: string;
  style_prompt: string;
  global_seed: number | null;
  created_at: string;
  updated_at: string;
}

export interface CreateProjectInput {
  name: string;
  description?: string;
  style_prompt?: string;
  global_seed?: number;
}

export interface UpdateProjectInput {
  name?: string;
  description?: string;
  style_prompt?: string;
  global_seed?: number | null;  // undefined=不修改, null=清空, number=设值
}

export interface ListProjectsOptions {
  limit?: number;
  offset?: number;
}
```

## 验收标准

```bash
# 1. 编译通过
cargo build -p mango-core

# 2. 单元测试全部通过
cargo test -p mango-core queries::project

# 3. ts-rs 类型生成（输出路径由 .cargo/config.toml 中 TS_RS_EXPORT_DIR 控制）
cargo test -p mango-core export_bindings
# → 检查 src/lib/bindings/types/ 下生成了 Project.ts 等文件
```

**测试用例清单：**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use std::path::Path;

    fn setup() -> Connection {
        open_sync(Path::new(":memory:")).unwrap()
    }

    #[test]
    fn test_create_project() {
        let conn = setup();
        let project = create(
            &conn,
            CreateProjectInput {
                name: "测试项目".into(),
                description: Some("描述".into()),
                style_prompt: None,
                global_seed: None,
            },
        )
        .unwrap();

        assert!(!project.id.is_empty());
        assert_eq!(project.name, "测试项目");
        assert_eq!(project.description, "描述");
        assert!(!project.created_at.is_empty());
        assert_eq!(project.created_at, project.updated_at);
    }

    #[test]
    fn test_create_project_empty_name_fails() {
        let conn = setup();
        let result = create(
            &conn,
            CreateProjectInput {
                name: "  ".into(),
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        );
        assert!(matches!(result, Err(CoreError::Validation(_))));
    }

    #[test]
    fn test_get_by_id_not_found() {
        let conn = setup();
        let result = get_by_id(&conn, "nonexistent");
        assert!(matches!(result, Err(CoreError::NotFound { .. })));
    }

    #[test]
    fn test_list_with_pagination() {
        let conn = setup();
        for i in 0..5 {
            create(
                &conn,
                CreateProjectInput {
                    name: format!("Project {i}"),
                    description: None,
                    style_prompt: None,
                    global_seed: None,
                },
            )
            .unwrap();
        }

        let page1 = list(&conn, ListProjectsOptions { limit: Some(2), offset: Some(0) }).unwrap();
        assert_eq!(page1.len(), 2);

        let page2 = list(&conn, ListProjectsOptions { limit: Some(2), offset: Some(2) }).unwrap();
        assert_eq!(page2.len(), 2);

        let all = list(&conn, ListProjectsOptions::default()).unwrap();
        assert_eq!(all.len(), 5);
    }

    #[test]
    fn test_update_project_partial() {
        let conn = setup();
        let project = create(
            &conn,
            CreateProjectInput {
                name: "Original".into(),
                description: Some("desc".into()),
                style_prompt: None,
                global_seed: None,
            },
        )
        .unwrap();

        let updated = update(
            &conn,
            &project.id,
            UpdateProjectInput {
                name: Some("Renamed".into()),
                description: None, // should not change
                style_prompt: None,
                global_seed: None,
            },
        )
        .unwrap();

        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.description, "desc"); // unchanged
    }

    #[test]
    fn test_update_project_empty_name_fails() {
        let conn = setup();
        let project = create(
            &conn,
            CreateProjectInput {
                name: "Test".into(),
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        )
        .unwrap();

        let result = update(
            &conn,
            &project.id,
            UpdateProjectInput {
                name: Some("  ".into()),
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        );
        assert!(matches!(result, Err(CoreError::Validation(_))));
    }

    #[test]
    fn test_delete_project() {
        let conn = setup();
        let project = create(
            &conn,
            CreateProjectInput {
                name: "To Delete".into(),
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        )
        .unwrap();

        delete(&conn, &project.id).unwrap();
        assert!(matches!(
            get_by_id(&conn, &project.id),
            Err(CoreError::NotFound { .. })
        ));
    }

    #[test]
    fn test_delete_nonexistent_fails() {
        let conn = setup();
        let result = delete(&conn, "nonexistent");
        assert!(matches!(result, Err(CoreError::NotFound { .. })));
    }
}
```

## 依赖关系

- **前置**：SPEC-03（需要 DB 连接和 schema）
- **后续**：
  - SPEC-05（Tauri IPC）调用这些 CRUD 函数
  - SPEC-06（CLI 骨架）调用这些 CRUD 函数
  - 后续迭代的其他实体 CRUD 参考此模式

## 与原版本的变更摘要

| 变更项 | 旧方案 | 新方案 | 原因 |
|--------|--------|--------|------|
| 时间生成 | Rust 侧 `chrono::Utc::now().format(...)` | INSERT 不传时间列，依赖 DB DEFAULT；UPDATE 用 SQL `datetime('now')` | 消除 Rust/DB 时间格式不一致风险 |
| CoreError | 整合 `DbError` + `rusqlite::Error` + `tokio_rusqlite::Error`（编译不过） | 仅含 `rusqlite::Error`，`DbError` 留在连接层 | 避免 `#[from]` 冲突和递归类型 |
| update 实现 | COALESCE + CASE WHEN hack | 动态 SQL 构建，只更新非 None 字段 | 语义清晰，可扩展，无 dead code |
| global_seed 更新 | `Option<i64>` 无法表达"清空为 NULL" | `Option<Option<i64>>`（None=不改, Some(None)=清空, Some(v)=设值） | 完整覆盖 nullable 字段的三态语义 |
| list 接口 | 无分页，返回全量 | 接受 `ListProjectsOptions { limit, offset }` | 为后续实体建立可扩展模式 |
| ts-rs 版本 | `ts-rs = "12"` 未入 workspace | `ts-rs = { version = "12", features = ["chrono-impl", "uuid-impl"] }` 加入 workspace.dependencies | 统一依赖管理，预留类型导出能力 |
| ts-rs export 路径 | 硬编码 `export_to = "../../src/lib/bindings/types/"` | `#[ts(export)]` + `.cargo/config.toml` 持久化 `TS_RS_EXPORT_DIR` | 避免路径脆弱性，官方推荐方案 |
| 测试 setup | `tempfile::NamedTempFile`（有生命周期问题） | `:memory:` | 符合 testing rules，无生命周期隐患 |
| 测试排序验证 | `thread::sleep` 制造时间差 | 改为分页测试 | sleep 在测试中不可靠且拖慢速度 |
