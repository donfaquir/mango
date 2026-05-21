# SPEC-03: SQLite 全量 Schema 定义 + 迁移框架

## 概述

在 core crate 中建立完整的数据库层：定义全部 18 张表的 DDL、实现简洁的迁移执行器、提供同步/异步双 API。此模块是所有业务数据操作的基础。

## 技术方案

### 目录/文件结构

```
crates/core/src/
├── lib.rs
└── db/
    ├── mod.rs              # 模块导出 + Database 结构体
    ├── connection.rs       # 连接管理（同步 + 异步）
    ├── migrator.rs         # 迁移执行器
    ├── schema.rs           # 表结构常量（可选，用于文档化）
    └── migrations/
        └── 001_initial.sql # 首版全量 schema
```

### 依赖项

在 core 的 Cargo.toml 中（通过 workspace 引用）：
```toml
[dependencies]
rusqlite = { workspace = true }
tokio-rusqlite = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }
```

### 核心实现

#### 迁移执行器（migrator.rs）

```rust
use rusqlite::Connection;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MigrationError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("migration {version} failed: {reason}")]
    Failed { version: i64, reason: String },
}

/// 嵌入式迁移脚本
struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

/// 所有迁移脚本（编译时嵌入）
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial",
        sql: include_str!("migrations/001_initial.sql"),
    },
];

/// 执行迁移（每个迁移在独立事务中执行）
pub fn run_migrations(conn: &Connection) -> Result<(), MigrationError> {
    // 1. 创建 _migrations 表（如不存在）
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        );"
    )?;

    // 2. 查询已执行的最大版本
    let current_version: i64 = conn
        .query_row("SELECT COALESCE(MAX(version), 0) FROM _migrations", [], |r| r.get(0))
        .unwrap_or(0);

    // 3. 在事务中执行未应用的迁移
    for migration in MIGRATIONS.iter().filter(|m| m.version > current_version) {
        tracing::info!("applying migration {} ({})", migration.version, migration.name);

        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(migration.sql)?;
        tx.execute(
            "INSERT INTO _migrations (version, name) VALUES (?1, ?2)",
            rusqlite::params![migration.version, migration.name],
        )?;
        tx.commit()?;
    }

    Ok(())
}
```

#### 连接管理（connection.rs）

```rust
use rusqlite::Connection;
use tokio_rusqlite::Connection as AsyncConnection;
use std::path::Path;

#[derive(thiserror::Error, Debug)]
pub enum DbError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("async sqlite error: {0}")]
    AsyncSqlite(#[from] tokio_rusqlite::Error),
    #[error("migration error: {0}")]
    Migration(#[from] super::migrator::MigrationError),
}

/// 初始化 PRAGMA（WAL 模式、外键、性能优化）
fn init_pragmas(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;
         PRAGMA synchronous = NORMAL;"
    )
}

/// 同步数据库连接（CLI 使用）
pub fn open_sync(path: &Path) -> Result<Connection, DbError> {
    let conn = Connection::open(path)?;
    init_pragmas(&conn)?;
    super::migrator::run_migrations(&conn)?;
    Ok(conn)
}

/// 异步数据库连接（Tauri 使用）
pub async fn open_async(path: &Path) -> Result<AsyncConnection, DbError> {
    let path = path.to_path_buf();
    let conn = AsyncConnection::open(&path).await?;
    conn.call(|conn| {
        init_pragmas(conn).map_err(tokio_rusqlite::Error::Rusqlite)?;
        super::migrator::run_migrations(conn).map_err(|e| {
            tokio_rusqlite::Error::Other(Box::new(e))
        })?;
        Ok(())
    }).await?;
    Ok(conn)
}
```

#### 模块导出（db/mod.rs）

```rust
pub mod connection;
pub mod migrator;

pub use connection::{open_sync, open_async, DbError};
```

#### 首版迁移脚本（migrations/001_initial.sql）

```sql
-- ============================================================
-- Mango 全量 Schema V1
-- ============================================================

-- 项目
CREATE TABLE project (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    style_prompt TEXT NOT NULL DEFAULT '',
    global_seed INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 集/章节
CREATE TABLE episode (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    order_index INTEGER NOT NULL DEFAULT 0,
    script_text TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_episode_project ON episode(project_id);

-- 分镜
CREATE TABLE shot (
    id TEXT PRIMARY KEY,
    episode_id TEXT NOT NULL REFERENCES episode(id) ON DELETE CASCADE,
    order_index INTEGER NOT NULL DEFAULT 0,
    summary TEXT NOT NULL DEFAULT '',
    duration_sec REAL,
    camera_angle TEXT NOT NULL DEFAULT '',
    shot_type TEXT NOT NULL DEFAULT '',
    mood TEXT NOT NULL DEFAULT '',
    dialogue TEXT NOT NULL DEFAULT '',
    video_prompt TEXT NOT NULL DEFAULT '',
    image_prompt TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','ready','generating','done')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_shot_episode ON shot(episode_id);

-- 角色（避免使用 SQL 保留字 "character"）
CREATE TABLE character_profile (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    appearance_prompt TEXT NOT NULL DEFAULT '',
    reference_image_path TEXT,
    voice_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_character_project ON character_profile(project_id);

-- 场景
CREATE TABLE scene (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    environment_prompt TEXT NOT NULL DEFAULT '',
    reference_image_path TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_scene_project ON scene(project_id);

-- 道具
CREATE TABLE prop (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    reference_image_path TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_prop_project ON prop(project_id);

-- 服装
CREATE TABLE costume (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    character_id TEXT NOT NULL REFERENCES character_profile(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    reference_image_path TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_costume_project ON costume(project_id);
CREATE INDEX idx_costume_character ON costume(character_id);

-- 素材文件
CREATE TABLE asset (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    shot_id TEXT REFERENCES shot(id) ON DELETE SET NULL,
    asset_type TEXT NOT NULL CHECK(asset_type IN ('image','video','audio','script')),
    original_name TEXT NOT NULL,
    file_path TEXT NOT NULL,
    thumbnail_path TEXT,
    file_size INTEGER NOT NULL DEFAULT 0,
    content_hash TEXT,
    metadata_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_asset_project ON asset(project_id);
CREATE INDEX idx_asset_shot ON asset(shot_id);

-- 模型提供商
CREATE TABLE provider (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    base_url TEXT NOT NULL DEFAULT '',
    auth_type TEXT NOT NULL DEFAULT 'api_key' CHECK(auth_type IN ('api_key','oauth')),
    docs_url TEXT NOT NULL DEFAULT ''
);

-- 模型
CREATE TABLE model (
    id TEXT PRIMARY KEY,
    provider_id TEXT NOT NULL REFERENCES provider(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    model_type TEXT NOT NULL CHECK(model_type IN ('text','image','video','audio')),
    capabilities_json TEXT,
    default_params_json TEXT
);
CREATE INDEX idx_model_provider ON model(provider_id);

-- API 账号
CREATE TABLE api_account (
    id TEXT PRIMARY KEY,
    provider_id TEXT NOT NULL REFERENCES provider(id) ON DELETE CASCADE,
    label TEXT NOT NULL DEFAULT '',
    api_key_ref TEXT NOT NULL,
    usage_quota INTEGER,
    usage_used INTEGER NOT NULL DEFAULT 0,
    last_used_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_api_account_provider ON api_account(provider_id);

-- 生成任务
CREATE TABLE generation_task (
    id TEXT PRIMARY KEY,
    shot_id TEXT REFERENCES shot(id) ON DELETE SET NULL,
    provider_id TEXT NOT NULL REFERENCES provider(id) ON DELETE RESTRICT,
    model_id TEXT NOT NULL REFERENCES model(id) ON DELETE RESTRICT,
    account_id TEXT NOT NULL REFERENCES api_account(id) ON DELETE RESTRICT,
    task_type TEXT NOT NULL CHECK(task_type IN ('text','image','video','audio')),
    params_json TEXT NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','running','success','failed','cancelled')),
    result_asset_id TEXT REFERENCES asset(id) ON DELETE SET NULL,
    external_task_id TEXT,
    started_at TEXT,
    finished_at TEXT,
    error_message TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_task_shot ON generation_task(shot_id);
CREATE INDEX idx_task_status ON generation_task(status);
CREATE INDEX idx_task_created ON generation_task(created_at);

-- 画布布局
CREATE TABLE canvas_layout (
    id TEXT PRIMARY KEY,
    episode_id TEXT NOT NULL REFERENCES episode(id) ON DELETE CASCADE,
    nodes_json TEXT NOT NULL DEFAULT '[]',
    edges_json TEXT NOT NULL DEFAULT '[]',
    viewport_json TEXT NOT NULL DEFAULT '{}',
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE UNIQUE INDEX idx_canvas_episode ON canvas_layout(episode_id);

-- 集版本检查点
CREATE TABLE episode_checkpoint (
    id TEXT PRIMARY KEY,
    episode_id TEXT NOT NULL REFERENCES episode(id) ON DELETE CASCADE,
    version_number INTEGER NOT NULL,
    label TEXT,
    trigger_type TEXT NOT NULL DEFAULT 'manual' CHECK(trigger_type IN ('auto','manual')),
    script_text TEXT NOT NULL DEFAULT '',
    shots_json TEXT NOT NULL DEFAULT '[]',
    canvas_nodes_json TEXT NOT NULL DEFAULT '[]',
    canvas_edges_json TEXT NOT NULL DEFAULT '[]',
    canvas_viewport_json TEXT NOT NULL DEFAULT '{}',
    change_summary TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_checkpoint_episode ON episode_checkpoint(episode_id);

-- ============================================================
-- 关联表
-- ============================================================

CREATE TABLE shot_character (
    shot_id TEXT NOT NULL REFERENCES shot(id) ON DELETE CASCADE,
    character_id TEXT NOT NULL REFERENCES character_profile(id) ON DELETE CASCADE,
    PRIMARY KEY (shot_id, character_id)
);

CREATE TABLE shot_scene (
    shot_id TEXT NOT NULL REFERENCES shot(id) ON DELETE CASCADE,
    scene_id TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
    PRIMARY KEY (shot_id, scene_id)
);

CREATE TABLE shot_prop (
    shot_id TEXT NOT NULL REFERENCES shot(id) ON DELETE CASCADE,
    prop_id TEXT NOT NULL REFERENCES prop(id) ON DELETE CASCADE,
    PRIMARY KEY (shot_id, prop_id)
);
```

> **表名变更说明**：`character` 改为 `character_profile`，因为 `CHARACTER` 是 SQL 标准保留字，避免拼写 SQL 时需要引号转义，也利于未来可能的跨数据库迁移。

> **表数量统计**：14 张实体表（project, episode, shot, character_profile, scene, prop, costume, asset, provider, model, api_account, generation_task, canvas_layout, episode_checkpoint）+ 3 张关联表（shot_character, shot_scene, shot_prop）= **17 张业务表** + 1 张 `_migrations` 元数据表。

## 接口定义

```rust
// 公开 API（crates/core/src/db/mod.rs）

/// 打开同步连接（自动执行迁移）
pub fn open_sync(path: &Path) -> Result<Connection, DbError>;

/// 打开异步连接（自动执行迁移）
pub async fn open_async(path: &Path) -> Result<AsyncConnection, DbError>;

/// 错误类型
pub enum DbError {
    Sqlite(rusqlite::Error),
    AsyncSqlite(tokio_rusqlite::Error),
    Migration(MigrationError),
}
```

## 验收标准

```bash
# 1. 编译通过
cargo build -p mango-core

# 2. 单元测试 — 迁移执行
cargo test -p mango-core db::

# 测试内容：
# - open_sync 首次调用创建所有表
# - open_sync 重复调用不报错（幂等）
# - 检查全部 17 张业务表存在
# - 外键关系生效：插入无效 FK 触发错误
# - WAL 模式已启用
# - busy_timeout 和 synchronous PRAGMA 生效

# 3. 异步连接测试
cargo test -p mango-core db::connection::test_async
```

**单元测试伪代码：**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_migrations_create_all_tables() {
        let tmp = NamedTempFile::new().unwrap();
        let conn = open_sync(tmp.path()).unwrap();

        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE '_%' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(tables.len(), 17);
    }

    #[test]
    fn test_migrations_idempotent() {
        let tmp = NamedTempFile::new().unwrap();
        let conn = open_sync(tmp.path()).unwrap();
        drop(conn);
        let _conn = open_sync(tmp.path()).unwrap();
    }

    #[test]
    fn test_foreign_keys_enforced() {
        let tmp = NamedTempFile::new().unwrap();
        let conn = open_sync(tmp.path()).unwrap();
        let result = conn.execute(
            "INSERT INTO episode (id, project_id, title) VALUES ('e1', 'nonexistent', 'test')",
            [],
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_wal_mode_enabled() {
        let tmp = NamedTempFile::new().unwrap();
        let conn = open_sync(tmp.path()).unwrap();
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
    }

    #[test]
    fn test_pragmas_configured() {
        let tmp = NamedTempFile::new().unwrap();
        let conn = open_sync(tmp.path()).unwrap();

        let timeout: i64 = conn
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .unwrap();
        assert_eq!(timeout, 5000);

        let sync: i64 = conn
            .query_row("PRAGMA synchronous", [], |row| row.get(0))
            .unwrap();
        assert_eq!(sync, 1); // NORMAL = 1
    }

    #[tokio::test]
    async fn test_async_connection() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let conn = open_async(&path).await.unwrap();

        let count: i64 = conn.call(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
                [],
                |row| row.get(0),
            ).map_err(tokio_rusqlite::Error::Rusqlite)
        }).await.unwrap();

        assert!(count >= 17);
    }

    #[test]
    fn test_generation_task_restrict_delete() {
        let tmp = NamedTempFile::new().unwrap();
        let conn = open_sync(tmp.path()).unwrap();

        // Insert provider, model, account, then a task referencing them
        conn.execute_batch("
            INSERT INTO provider (id, name) VALUES ('p1', 'TestProvider');
            INSERT INTO model (id, provider_id, name, model_type) VALUES ('m1', 'p1', 'TestModel', 'image');
            INSERT INTO api_account (id, provider_id, api_key_ref) VALUES ('a1', 'p1', 'ref');
            INSERT INTO generation_task (id, provider_id, model_id, account_id, task_type)
                VALUES ('t1', 'p1', 'm1', 'a1', 'image');
        ").unwrap();

        // Deleting provider should fail due to RESTRICT
        let result = conn.execute("DELETE FROM provider WHERE id = 'p1'", []);
        assert!(result.is_err());
    }
}
```

## 依赖关系

- **前置**：SPEC-01（core crate 结构）
- **后续**：
  - SPEC-04（Project CRUD）直接使用此模块的连接和表
  - SPEC-05（Tauri IPC）使用 `open_async`
  - SPEC-06（CLI 骨架）使用 `open_sync`

## 与旧版本的变更摘要

| 变更项 | 旧方案 | 新方案 | 原因 |
|--------|--------|--------|------|
| 迁移事务 | SQL 执行和版本记录分离 | 每个迁移包在 `unchecked_transaction` 中 | 防止 SQL 成功但记录失败导致状态不一致 |
| `updated_at` 字段 | 仅 project、canvas_layout | episode、shot、character_profile、scene、prop、costume、asset 均添加 | 这些实体都会被编辑，需要追踪修改时间 |
| `character` 表名 | `character` | `character_profile` | `CHARACTER` 是 SQL 保留字，避免引号转义 |
| generation_task FK | 未指定 ON DELETE | provider/model/account 使用 `ON DELETE RESTRICT` | 防止误删仍被任务引用的 provider/model/account |
| generation_task 索引 | 仅 status | 新增 `idx_task_created` | 按时间查询任务列表是常见操作 |
| PRAGMA | WAL + foreign_keys | 新增 `busy_timeout=5000` + `synchronous=NORMAL` | 防并发 BUSY 错误 + WAL 下推荐的性能设置 |
| async 错误处理 | `map_err(\|e\| e.into())` | 显式 `tokio_rusqlite::Error::Rusqlite` / `Error::Other` | 确保类型转换编译通过 |
| 表数量 | 注释写 18 但计算混乱 | 明确 17 张业务表 + 1 张 _migrations | 修正断言和文档 |
