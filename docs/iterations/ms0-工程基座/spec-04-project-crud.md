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

core crate 新增（workspace 引用）：
```toml
uuid = { workspace = true }
chrono = { workspace = true }
ts-rs = "12"
```

dev-dependencies：
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
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("not found: {entity} with id '{id}'")]
    NotFound { entity: &'static str, id: String },

    #[error("validation error: {0}")]
    Validation(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;
```

#### 数据模型（models/project.rs）

```rust
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/types/")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: String,
    pub style_prompt: String,
    pub global_seed: Option<i64>,
    pub created_at: String,  // ISO 8601 格式
    pub updated_at: String,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/types/")]
pub struct CreateProjectInput {
    pub name: String,
    pub description: Option<String>,
    pub style_prompt: Option<String>,
    pub global_seed: Option<i64>,
}

#[derive(Debug, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/types/")]
pub struct UpdateProjectInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub style_prompt: Option<String>,
    pub global_seed: Option<i64>,
}
```

#### 查询实现（db/queries/project.rs）

```rust
use rusqlite::{Connection, params};
use uuid::Uuid;
use crate::error::{CoreError, Result};
use crate::models::project::{Project, CreateProjectInput, UpdateProjectInput};

/// 创建项目
pub fn create(conn: &Connection, input: CreateProjectInput) -> Result<Project> {
    if input.name.trim().is_empty() {
        return Err(CoreError::Validation("name cannot be empty".into()));
    }

    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    conn.execute(
        "INSERT INTO project (id, name, description, style_prompt, global_seed, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            input.name.trim(),
            input.description.unwrap_or_default(),
            input.style_prompt.unwrap_or_default(),
            input.global_seed,
            now,
            now,
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
        other => CoreError::Database(other),
    })
}

/// 获取所有项目（按创建时间倒序）
pub fn list(conn: &Connection) -> Result<Vec<Project>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, style_prompt, global_seed, created_at, updated_at
         FROM project ORDER BY created_at DESC"
    )?;

    let projects = stmt.query_map([], |row| {
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

/// 更新项目
pub fn update(conn: &Connection, id: &str, input: UpdateProjectInput) -> Result<Project> {
    // 先确认存在
    let _ = get_by_id(conn, id)?;

    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    // 构建动态 SET 子句
    let mut sets = vec!["updated_at = ?1"];
    let mut param_index = 2u32;

    if input.name.is_some() {
        if input.name.as_ref().unwrap().trim().is_empty() {
            return Err(CoreError::Validation("name cannot be empty".into()));
        }
    }

    // 使用固定位置参数的简化方案
    conn.execute(
        "UPDATE project SET
            name = COALESCE(?2, name),
            description = COALESCE(?3, description),
            style_prompt = COALESCE(?4, style_prompt),
            global_seed = CASE WHEN ?5 = 1 THEN ?6 ELSE global_seed END,
            updated_at = ?1
         WHERE id = ?7",
        params![
            now,
            input.name,
            input.description,
            input.style_prompt,
            input.global_seed.is_some() as i32,
            input.global_seed,
            id,
        ],
    )?;

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

## 接口定义

```rust
// 公开 API（crates/core/src/db/queries/project.rs）

pub fn create(conn: &Connection, input: CreateProjectInput) -> Result<Project>;
pub fn get_by_id(conn: &Connection, id: &str) -> Result<Project>;
pub fn list(conn: &Connection) -> Result<Vec<Project>>;
pub fn update(conn: &Connection, id: &str, input: UpdateProjectInput) -> Result<Project>;
pub fn delete(conn: &Connection, id: &str) -> Result<()>;
```

**TypeScript 类型（自动生成）：**
```typescript
// src/lib/bindings/types/Project.ts
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
  global_seed?: number;
}
```

## 验收标准

```bash
# 1. 编译通过
cargo build -p mango-core

# 2. 单元测试全部通过
cargo test -p mango-core queries::project

# 3. ts-rs 类型生成
cargo test -p mango-core export_bindings
# → 检查 src/lib/bindings/types/ 下生成了 Project.ts 等文件
```

**测试用例清单：**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use tempfile::NamedTempFile;

    fn setup() -> Connection {
        let tmp = NamedTempFile::new().unwrap();
        open_sync(tmp.path()).unwrap()
    }

    #[test]
    fn test_create_project() {
        let conn = setup();
        let project = create(&conn, CreateProjectInput {
            name: "测试项目".into(),
            description: Some("描述".into()),
            style_prompt: None,
            global_seed: None,
        }).unwrap();

        assert!(!project.id.is_empty());
        assert_eq!(project.name, "测试项目");
        assert_eq!(project.description, "描述");
    }

    #[test]
    fn test_create_project_empty_name_fails() {
        let conn = setup();
        let result = create(&conn, CreateProjectInput {
            name: "  ".into(),
            description: None,
            style_prompt: None,
            global_seed: None,
        });
        assert!(matches!(result, Err(CoreError::Validation(_))));
    }

    #[test]
    fn test_get_by_id_not_found() {
        let conn = setup();
        let result = get_by_id(&conn, "nonexistent");
        assert!(matches!(result, Err(CoreError::NotFound { .. })));
    }

    #[test]
    fn test_list_order_by_created_desc() {
        let conn = setup();
        create(&conn, CreateProjectInput { name: "A".into(), ..Default::default() }).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        create(&conn, CreateProjectInput { name: "B".into(), ..Default::default() }).unwrap();

        let projects = list(&conn).unwrap();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].name, "B"); // 最新的在前
    }

    #[test]
    fn test_update_project() {
        let conn = setup();
        let project = create(&conn, CreateProjectInput { name: "Original".into(), ..Default::default() }).unwrap();

        let updated = update(&conn, &project.id, UpdateProjectInput {
            name: Some("Renamed".into()),
            description: None,
            style_prompt: None,
            global_seed: None,
        }).unwrap();

        assert_eq!(updated.name, "Renamed");
        assert!(updated.updated_at > project.updated_at);
    }

    #[test]
    fn test_delete_project() {
        let conn = setup();
        let project = create(&conn, CreateProjectInput { name: "To Delete".into(), ..Default::default() }).unwrap();

        delete(&conn, &project.id).unwrap();
        assert!(matches!(get_by_id(&conn, &project.id), Err(CoreError::NotFound { .. })));
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
