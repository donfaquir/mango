# SPEC-13: API 账号管理 + keyring + Provider/Model 预置

## 概述

实现 ApiAccount 的 CRUD：API Key 通过系统 keyring（macOS Keychain / Windows Credential Manager / Linux Secret Service）安全存储，DB 仅记录 `api_key_ref`（保留列，恒为 `keyring::user_id(account_id)`）与 `key_last4`（用于列表展示）。同时落地 Provider / Model 的启动期 seed，让 UI 一上线就有可灵 / 即梦两个 Provider 可选。再补齐 `mango account` CLI 子命令。

对应 `开发任务.md` 任务 8（Provider/Model 预置）+ 任务 9（ApiAccount CRUD + keyring）+ 任务 11 中的 account 部分。

## 设计目标 / 非目标

**目标**：
- API Key 永远不写入 SQLite；只通过 keyring 读写
- 账号列表加载时**不调** keyring（性能 + 安全），靠预存的 `key_last4`
- Provider / Model 启动 seed，幂等；系统拥有列（name/base_url/docs_url）随版本可被覆盖
- seed 复用 spec-10 的 `startup::initialize` 统一入口（**不**额外引入启动钩子）
- 抽象 `KeyringStore` trait，单测注入 in-memory mock；prod 走 `keyring` crate
- 提供 `verify_storage` IPC（仅校验 keyring 中存有非空密钥；命名上与"测试网络连通"严格区分，避免后续替换误伤）
- CLI `mango account add/list/remove/verify`；`add --key` 缺省时从 stdin 读取

**非目标**：
- 不实现真实的 API 连接测试（等 MS2 Provider trait 落地，再新增 `test_connection` IPC，与 `verify_storage` 并存）
- 不实现 keyring 不可用时的加密文件降级方案（MVP 直接报错给用户，文档说明）
- 不实现用量统计 UI（schema `usage_quota` / `usage_used` 已就位，MS2/MS4 再用）
- 不实现多账号配额轮询（MS4）
- 不修改/删除 `api_key_ref` 列（保留为审计字段，恒为 `keyring::user_id(id)`；V2 若更换 keyring 命名约定时迁移有用）
- 不提供 CLI update 子命令（密钥替换走 remove+add；label 修改不常用，留给 GUI）

## 技术方案

### 目录/文件结构

```
crates/core/src/
├── account/                            # 新增模块
│   ├── mod.rs
│   ├── keyring.rs                      # KeyringStore trait + 系统 keyring 实现
│   └── service.rs                      # ApiAccount 业务流（组合 keyring + queries）
├── seed/                               # 新增模块
│   ├── mod.rs
│   └── providers.rs                    # 静态 Provider/Model 数据 + apply()
├── models/
│   ├── api_account.rs
│   └── provider.rs                     # Provider + Model 类型
├── error.rs                            # 新增 CoreError::Keyring 变体
├── db/
│   ├── migrator.rs                     # 注册 003 迁移
│   ├── migrations/
│   │   └── 003_add_api_account_key_last4.sql
│   └── queries/
│       ├── api_account.rs
│       └── provider.rs                 # list_providers / list_models
└── lib.rs                              # pub mod account; pub mod seed;

src-tauri/src/commands/
├── account.rs
└── provider.rs

src/hooks/
├── useAccounts.ts
└── useProviders.ts

crates/cli/src/commands/
└── account.rs
```

### 依赖项变更

**新增 Rust 依赖**（需批准）：
```toml
# workspace 根 Cargo.toml
keyring = "3"
rpassword = "7"                       # 仅 crates/cli 用，安全读取 stdin 不回显
```

> 已在 `docs/技术选型.md` §2.1 列入计划（"密钥存储 / keyring 3.x"）。实施前用 `WebSearch` 核对最新 stable。

`crates/core/Cargo.toml` 引用：`keyring = { workspace = true }`。
`crates/cli/Cargo.toml` 引用：`rpassword = { workspace = true }`。

### Schema 变更

**`003_add_api_account_key_last4.sql`**：
```sql
ALTER TABLE api_account ADD COLUMN key_last4 TEXT NOT NULL DEFAULT '';
```

> **设计说明**：默认空串而非 NULL，避免 SELECT 时分支处理。MS0 没有 api_account 行（功能未上线），所以"既有行 backfill"问题不存在。仅 ADD 不 DROP——`api_key_ref` 列保留（见下文）。

### 错误类型扩展

`crates/core/src/error.rs` 在 spec-10 已加 `Io` 的基础上再加：
```rust
#[derive(Error, Debug)]
pub enum CoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("keyring error: {0}")]
    Keyring(String),                            // 新增

    #[error("not found: {entity} with id '{id}'")]
    NotFound { entity: &'static str, id: String },

    #[error("validation error: {0}")]
    Validation(String),
}
```

> 不直接 `#[from] keyring::Error`——keyring crate 的 Error 在不同平台 variants 不同，不利于 IPC 序列化。统一存字符串。

### 数据模型

`models/api_account.rs`：
```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ApiAccount {
    pub id: String,
    pub provider_id: String,
    pub label: String,
    pub key_last4: String,                       // 仅用于展示
    #[specta(type = Option<specta_typescript::Number>)]
    pub usage_quota: Option<i64>,
    #[specta(type = specta_typescript::Number)]
    pub usage_used: i64,
    pub last_used_at: Option<String>,
    pub created_at: String,
    // api_key_ref 仅 backend 使用，不导出给前端
}

#[derive(Debug, Deserialize, Type)]
pub struct CreateApiAccountInput {
    pub provider_id: String,
    pub label: String,
    pub api_key: String,                         // 明文，提交后立即写入 keyring 不再保留
}

#[derive(Debug, Deserialize, Type)]
pub struct UpdateApiAccountInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Some(s) 表示替换密钥；None 表示不动
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}
```

`models/provider.rs`：
```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub auth_type: String,
    pub docs_url: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Model {
    pub id: String,
    pub provider_id: String,
    pub name: String,
    pub model_type: String,              // 'image' | 'video'
    pub created_at: String,
}
```

> **设计说明**：`ApiAccount` 故意不包含 `api_key_ref` 字段——前端没有任何理由知道这个值，把它从 `Type` 层屏蔽避免误用。后端写入时恒等于 `keyring::user_id(account.id)`，纯粹的审计副本。

### keyring 抽象（`crates/core/src/account/keyring.rs`）

```rust
use crate::error::{CoreError, Result};

const SERVICE: &str = "com.mango.app";

/// 构建一个账号在 keyring 中的 user 标识。
/// 同时用作 `api_account.api_key_ref` 列的值（审计冗余）。
pub fn user_id(account_id: &str) -> String {
    format!("api_account:{account_id}")
}

/// 抽象的 keyring 接口，便于单测注入 in-memory 实现。
pub trait KeyringStore: Send + Sync {
    fn store(&self, account_id: &str, key: &str) -> Result<()>;
    fn fetch(&self, account_id: &str) -> Result<String>;
    fn remove(&self, account_id: &str) -> Result<()>;
}

/// 系统 keyring 实现（生产用）。
pub struct SystemKeyring;

impl SystemKeyring {
    fn entry(account_id: &str) -> Result<keyring::Entry> {
        keyring::Entry::new(SERVICE, &user_id(account_id))
            .map_err(|e| CoreError::Keyring(e.to_string()))
    }
}

impl KeyringStore for SystemKeyring {
    fn store(&self, account_id: &str, key: &str) -> Result<()> {
        Self::entry(account_id)?
            .set_password(key)
            .map_err(|e| CoreError::Keyring(e.to_string()))
    }

    fn fetch(&self, account_id: &str) -> Result<String> {
        Self::entry(account_id)?
            .get_password()
            .map_err(|e| CoreError::Keyring(e.to_string()))
    }

    fn remove(&self, account_id: &str) -> Result<()> {
        match Self::entry(account_id)?.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),  // 幂等
            Err(e) => Err(CoreError::Keyring(e.to_string())),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct InMemoryKeyring(Mutex<HashMap<String, String>>);

    impl KeyringStore for InMemoryKeyring {
        fn store(&self, id: &str, key: &str) -> Result<()> {
            self.0.lock().unwrap().insert(id.to_string(), key.to_string());
            Ok(())
        }
        fn fetch(&self, id: &str) -> Result<String> {
            self.0.lock().unwrap().get(id).cloned()
                .ok_or_else(|| CoreError::Keyring(format!("not found: {id}")))
        }
        fn remove(&self, id: &str) -> Result<()> {
            self.0.lock().unwrap().remove(id);
            Ok(())
        }
    }
}
```

### 业务流（`crates/core/src/account/service.rs`）

```rust
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::account::keyring::{user_id, KeyringStore};
use crate::error::{CoreError, Result};
use crate::models::api_account::{ApiAccount, CreateApiAccountInput, UpdateApiAccountInput};
use crate::db::queries::api_account as queries;

pub fn create(
    conn: &Connection,
    keyring: &dyn KeyringStore,
    input: CreateApiAccountInput,
) -> Result<ApiAccount> {
    if input.api_key.is_empty() {
        return Err(CoreError::Validation("api_key cannot be empty".into()));
    }
    if input.label.trim().is_empty() {
        return Err(CoreError::Validation("label cannot be empty".into()));
    }

    let id = Uuid::new_v4().to_string();
    let key_last4 = last4(&input.api_key);
    let api_key_ref = user_id(&id);

    // 1. 先写 keyring（失败则放弃，DB 不留行）
    keyring.store(&id, &input.api_key)?;

    // 2. INSERT DB；失败则回滚 keyring（best-effort）
    let result = conn.execute(
        "INSERT INTO api_account
            (id, provider_id, label, api_key_ref, key_last4)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, input.provider_id, input.label.trim(), api_key_ref, key_last4],
    );
    if let Err(e) = result {
        if let Err(re) = keyring.remove(&id) {
            tracing::warn!("failed to rollback keyring entry for {id}: {re}");
        }
        return Err(e.into());
    }
    queries::get_by_id(conn, &id)
}

pub fn delete(conn: &Connection, keyring: &dyn KeyringStore, id: &str) -> Result<()> {
    // 先删 DB，再删 keyring；keyring 删除失败仅 warn
    let n = conn.execute("DELETE FROM api_account WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(CoreError::NotFound { entity: "api_account", id: id.to_string() });
    }
    if let Err(e) = keyring.remove(id) {
        tracing::warn!("keyring entry orphaned for {id}: {e}");
    }
    Ok(())
}

pub fn update(
    conn: &Connection,
    keyring: &dyn KeyringStore,
    id: &str,
    input: UpdateApiAccountInput,
) -> Result<ApiAccount> {
    if let Some(new_key) = input.api_key {
        if new_key.is_empty() {
            return Err(CoreError::Validation("api_key cannot be empty".into()));
        }
        keyring.store(id, &new_key)?;
        let last4 = last4(&new_key);
        conn.execute(
            "UPDATE api_account SET key_last4 = ?1 WHERE id = ?2",
            params![last4, id],
        )?;
    }
    if let Some(label) = input.label {
        conn.execute(
            "UPDATE api_account SET label = ?1 WHERE id = ?2",
            params![label.trim(), id],
        )?;
    }
    queries::get_by_id(conn, id)
}

/// 仅验证 keyring 中存在该账号的密钥且非空。
/// 与"测试网络连通"（test_connection，MS2 引入）严格区分；命名带 storage 强调这一点。
pub fn verify_storage(conn: &Connection, keyring: &dyn KeyringStore, id: &str) -> Result<()> {
    let _account = queries::get_by_id(conn, id)?;
    let key = keyring.fetch(id)?;
    if key.is_empty() {
        return Err(CoreError::Validation("stored api key is empty".into()));
    }
    Ok(())
}

fn last4(s: &str) -> String {
    let n = s.chars().count();
    if n <= 4 {
        s.to_string()
    } else {
        s.chars().skip(n - 4).collect()
    }
}
```

> **设计说明**：
> - 创建顺序"先 keyring 再 DB"——keyring 失败时绝对不留 DB 孤儿行；DB 失败时尽力清 keyring（失败仅 warn，不让用户看到内部清理细节）。
> - `last4` 用 `chars().count()` + `skip` 而非链式 reverse，处理 unicode 边界更清晰。
> - `verify_storage` 命名替代 v1 的 `test_connection`，避免 MS2 引入真正网络测试时复用同一命名造成误解。MS2 时新增 `test_connection` IPC（真实 HTTP ping），`verify_storage` 保留为快速本地校验。

### Provider / Model Seed（`crates/core/src/seed/providers.rs`）

```rust
use rusqlite::{params, Connection};
use crate::error::Result;

struct ProviderSeed {
    id: &'static str,
    name: &'static str,
    base_url: &'static str,
    docs_url: &'static str,
    models: &'static [ModelSeed],
}

struct ModelSeed {
    id: &'static str,
    name: &'static str,
    model_type: &'static str,           // 'image' | 'video'
}

const PROVIDERS: &[ProviderSeed] = &[
    ProviderSeed {
        id: "kling",
        name: "可灵",
        base_url: "",                    // 实施时从可灵官方文档填入
        docs_url: "",
        models: &[
            ModelSeed { id: "kling-image-v1", name: "可灵图像 v1", model_type: "image" },
            ModelSeed { id: "kling-video-v1", name: "可灵视频 v1", model_type: "video" },
        ],
    },
    ProviderSeed {
        id: "jimeng",
        name: "即梦",
        base_url: "",
        docs_url: "",
        models: &[
            ModelSeed { id: "jimeng-image-v1", name: "即梦图像 v1", model_type: "image" },
            ModelSeed { id: "jimeng-video-v1", name: "即梦视频 v1", model_type: "video" },
        ],
    },
];

/// 幂等应用 seed 数据。系统拥有列（name/base_url/docs_url/auth_type/model_type）随发布版本可被覆盖；
/// 用户不应该手动改 provider/model 表，否则升级会被回滚。
pub fn apply(conn: &Connection) -> Result<()> {
    for p in PROVIDERS {
        conn.execute(
            "INSERT INTO provider (id, name, base_url, auth_type, docs_url)
             VALUES (?1, ?2, ?3, 'api_key', ?4)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                base_url = excluded.base_url,
                auth_type = excluded.auth_type,
                docs_url = excluded.docs_url",
            params![p.id, p.name, p.base_url, p.docs_url],
        )?;
        for m in p.models {
            conn.execute(
                "INSERT INTO model (id, provider_id, name, model_type)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    model_type = excluded.model_type",
                params![m.id, p.id, m.name, m.model_type],
            )?;
        }
    }
    Ok(())
}
```

> **设计说明**：
> - 改 `DO UPDATE SET` 解决 v1 痛点："base_url 实施时留空 → 后续发版填好 → seed 因 DO NOTHING 永不覆盖"。系统拥有列每次启动都被强制对齐到代码里的 seed 数据。
> - 用户自有数据（`api_account` 引用的 `provider_id`）不受影响——只覆盖元数据列。
> - 每次启动跑一次：开销忽略不计（4 行 UPSERT，全部命中索引）。

### 启动期挂载

由 spec-10 的 `startup::initialize` 调用：
```rust
// crates/core/src/startup.rs
pub fn initialize(conn: &Connection, app_data_dir: &Path) -> Result<()> {
    backfill_project_roots(conn, app_data_dir)?;
    crate::seed::providers::apply(conn)?;     // 本 spec 提供
    Ok(())
}
```

Tauri 与 CLI 都通过 `startup::initialize` 间接触发 seed，**不**在各自启动代码里直接调 `seed::providers::apply`。

### Tauri command 层

`commands/account.rs`：注入 `&dyn KeyringStore`。`AppState` 增加 `keyring: Arc<dyn KeyringStore>`，在 `setup` 中初始化为 `Arc::new(SystemKeyring)`。

```rust
#[tauri::command]
#[specta::specta]
pub async fn create_api_account(
    state: State<'_, AppState>,
    input: CreateApiAccountInput,
) -> Result<ApiAccount, IpcError> {
    let keyring = state.keyring.clone();
    with_db(&state, move |conn| account::service::create(conn, keyring.as_ref(), input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_api_accounts(
    state: State<'_, AppState>,
    provider_id: Option<String>,
) -> Result<Vec<ApiAccount>, IpcError> { /* ... 仅查 DB ... */ }

#[tauri::command]
#[specta::specta]
pub async fn update_api_account(
    state: State<'_, AppState>,
    id: String,
    input: UpdateApiAccountInput,
) -> Result<ApiAccount, IpcError> { /* ... 注入 keyring ... */ }

#[tauri::command]
#[specta::specta]
pub async fn delete_api_account(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), IpcError> { /* ... 注入 keyring ... */ }

#[tauri::command]
#[specta::specta]
pub async fn verify_api_account_storage(    // 替代原 test_api_account
    state: State<'_, AppState>,
    id: String,
) -> Result<(), IpcError> { /* ... 注入 keyring ... */ }
```

`commands/provider.rs`：
```rust
#[tauri::command]
#[specta::specta]
pub async fn list_providers(state: State<'_, AppState>) -> Result<Vec<Provider>, IpcError>;

#[tauri::command]
#[specta::specta]
pub async fn list_models(
    state: State<'_, AppState>,
    provider_id: Option<String>,
) -> Result<Vec<Model>, IpcError>;
```

### 前端 hooks

`src/hooks/useAccounts.ts` / `useProviders.ts`：与既有 hooks 风格一致。

`useVerifyApiAccountStorage` 用 `useMutation`；按钮文案"校验密钥存储"（不是"测试连接"），UI 文案明确说"仅校验本机存储；网络连通测试在 MS2 上线"。

### CLI（`crates/cli/src/commands/account.rs`）

```rust
#[derive(Subcommand)]
pub enum AccountCommand {
    /// Add a new API account
    Add {
        #[arg(long)]
        provider_id: String,
        #[arg(long)]
        label: String,
        /// API key. If omitted, prompt securely from stdin.
        #[arg(long)]
        key: Option<String>,
    },
    /// List API accounts (key shown as ****<last4>)
    List {
        #[arg(long)]
        provider_id: Option<String>,
    },
    /// Remove an API account
    Remove { id: String },
    /// Verify the stored credential exists (does not call provider API)
    Verify { id: String },
    // 注：无 Update 子命令。修改 label 走 GUI；替换密钥走 remove+add。
}

pub fn execute(conn: &Connection, args: AccountArgs) -> anyhow::Result<()> {
    let keyring = mango_core::account::keyring::SystemKeyring;
    match args.command {
        AccountCommand::Add { provider_id, label, key } => {
            let api_key = match key {
                Some(k) => k,
                None => rpassword::prompt_password("API Key: ")?,
            };
            let acc = mango_core::account::service::create(
                conn, &keyring,
                CreateApiAccountInput { provider_id, label, api_key },
            )?;
            println!("{}", acc.id);
        }
        AccountCommand::List { provider_id } => {
            let rows = mango_core::db::queries::api_account::list(conn, provider_id)?;
            for a in rows {
                println!("{}\t{}\t****{}", a.id, a.label, a.key_last4);
            }
        }
        AccountCommand::Remove { id } => {
            mango_core::account::service::delete(conn, &keyring, &id)?;
        }
        AccountCommand::Verify { id } => {
            mango_core::account::service::verify_storage(conn, &keyring, &id)?;
            println!("ok");
        }
    }
    Ok(())
}
```

`main.rs` 添加 `Commands::Account(AccountArgs)`。

### 错误场景

| 场景 | 行为 |
|---|---|
| keyring 服务不可用（如 Linux 无 secret service） | `create` 返回 `CoreError::Keyring(...)`，前端 toast；MVP 不降级 |
| 删除时 keyring 已被外部清空 | `SystemKeyring::remove` 把 `NoEntry` 映射为 Ok，DB 行正常删 |
| 列表加载时 keyring 服务不可用 | 不影响（list 不读 keyring）；verify_storage 才会暴露问题 |
| seed apply 时 base_url 为空 | 不阻塞；MS2 真正调用 API 时才会发现 base_url 没填 |
| Provider/Model 在 seed 之外被用户误改/误删（CLI / 直接 SQL） | 下次启动自动覆盖回 seed 值；用户自有的 `api_account` 行不受影响 |

## 测试策略

### Rust 单元测试

`account/service.rs`：
- 用 `account::keyring::tests::InMemoryKeyring` 注入，不连真实 keyring（CI 跑得通）
- 覆盖：
  - `create_success_writes_keyring_and_db`
  - `create_db_failure_rolls_back_keyring`（mock 一个 INSERT 冲突）
  - `update_replaces_key_and_last4`
  - `delete_is_idempotent_when_keyring_missing`
  - `verify_storage_fails_on_empty_key`
- `last4_handles_short_and_unicode`：测 `last4("abc")` = `"abc"`, `last4("一二三四五")` 的字符（非字节）尾 4 位

`seed/providers.rs`：
- `apply_idempotent`：连续调两次后 provider/model 行数不增
- `apply_creates_expected_rows`：调用后 provider 表有 2 行、model 表有 4 行
- `apply_overwrites_system_owned_columns`：手动 UPDATE 把 kling.name 改成 'X' → 再调 apply → 验证 name 被恢复成 '可灵'

### 前端测试

`src/hooks/useAccounts.test.ts`：mock `commands.createApiAccount` 验证 mutation/invalidate 流；不真实调 keyring。

### 手动验证清单

- [ ] macOS：创建账号后在「钥匙串访问」搜 `com.mango.app` 见到条目
- [ ] Windows：在「凭据管理器」中可见
- [ ] 应用重启后，列表仍正确显示 `****<last4>`
- [ ] 删除账号后，钥匙串中该条目消失
- [ ] 编辑 provider 表把 name 改成 'X' → 重启应用 → name 恢复成代码 seed 值

## 验收标准

- [ ] `003_add_api_account_key_last4.sql` 加入迁移并幂等
- [ ] `Type` 层不导出 `api_key_ref` / `api_key`
- [ ] 创建账号时 API Key 写入系统 keyring，DB 仅存 `key_last4` 与恒等于 `keyring::user_id(id)` 的 `api_key_ref`
- [ ] 列表加载时**不调** keyring（log 验证）
- [ ] 删除账号时 keyring 条目同步清除
- [ ] `verify_api_account_storage` IPC 命名明确为"校验存储"；MS1 不暴露 `test_connection`，避免后续替换误用
- [ ] seed 用 `ON CONFLICT DO UPDATE` 覆盖系统拥有列；启动后 `provider` 表至少 2 行（kling + jimeng），`model` 表至少 4 行
- [ ] seed 通过 `startup::initialize` 统一入口调用（不在 setup 直接调）
- [ ] `mango account add` 不传 `--key` 时 stdin 安全读取（不回显）
- [ ] `mango account list` 仅显示 `****<last4>`，从不打印完整密钥
- [ ] `mango account verify <id>` 校验通过打印 `ok`
- [ ] CLI **不**提供 `account update` 子命令；文档注明替换密钥走 remove+add
- [ ] Rust 单测用 `InMemoryKeyring` 注入，无平台依赖
- [ ] 手动验证清单全过

## 依赖关系

- **前置**：spec-10（共享 `startup::initialize` 入口 + `CoreError::Io`）；不依赖 spec-11/12
- **并行**：可与 spec-11 / spec-12 并行实施（无代码耦合）
- **后续**：
  - spec-14（账号管理 UI）：使用本 spec 的 hooks
  - MS2（Provider 对接）：
    - 新增 `test_connection` IPC（真实 HTTP ping，区别于 `verify_storage`）
    - 调用方根据 `api_account.id` 用 `KeyringStore::fetch` 取出密钥发请求

## 与原任务文档的差异

`开发任务.md` 任务 9 验收"列表展示不暴露完整密钥（仅显示后 4 位）"原文意思是每次列表加载都从 keyring 取密钥再截取。本 spec 改为：创建/更新时把 last4 存入 DB（schema 003 迁移），列表加载完全不碰 keyring。理由：
- 性能：N 个账号 N 次 keyring 调用，跨平台都是同步 IPC，列表卡顿
- 安全：减少 keyring 读写次数 = 减少授权对话框出现次数
- 实施成本：003 迁移是一行 ALTER TABLE，几乎零成本

任务 10"可选：连接测试按钮"在本 spec 拆为两步：MS1 实现 `verify_storage`（仅本机校验），MS2 实现 `test_connection`（真实 HTTP）。命名上严格区分避免后续替换破坏 IPC 契约。
