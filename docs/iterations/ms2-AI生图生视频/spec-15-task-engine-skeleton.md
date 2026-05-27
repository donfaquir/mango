# SPEC-15: 任务引擎骨架与 ModelProvider 抽象

## 概述

为 MS2 引入 AI 生成任务的全链路骨架。定义统一的 `ModelProvider` trait、构建内存任务引擎（状态机 + 轮询 + 并发控制 + 启动期孤儿恢复）、通过 Tauri event 把任务状态变更实时推送到前端。

本 spec **不接入任何真实模型**——百炼 provider 由 spec-17 实现，OSS 上传层由 spec-16 实现，UI 由 spec-18 实现，CLI 由 spec-19 实现。本 spec 提供一个 `StubProvider`（仅 `#[cfg(test)]` 与 dev 命令开关启用），让任务引擎能在没有外部 API 的情况下跑通端到端：提交 → 轮询 → 推事件 → 状态落库 → 前端 invalidate query。

后续 spec 都在本 spec 的 trait 与 `TaskEngineHandle` 上叠加实现，**接口面一旦定型不可轻易变**——任何调整需返工 spec-16/17/18/19。

> ⚠️ **破坏性变更**：`crates/core/src/lib.rs` 新增 `pub mod provider; pub mod task_engine;`；`src-tauri/src/state.rs` 的 `AppState` 新增 `task_engine: Arc<TaskEngineHandle>` 字段；`startup::initialize` 新增「重置孤儿 running 任务」步骤。

## 设计目标 / 非目标

**目标**：

- ModelProvider trait 定义（submit/poll/cancel/download）+ `GenerationParams` 输入结构 + `ProviderTaskStatus` 输出枚举
- `task_engine` 模块：内存调度器，通过 `Arc<dyn ModelProvider>` 注册表按 `provider_id` 路由；最简并发控制（全局信号量 = 4）
- 状态机：`Pending → Running → Success / Failed / Cancelled`，DB 状态实时持久化 + tokio mpsc channel 通知 Tauri event
- `models/generation_task.rs` + `db/queries/generation_task.rs`：CRUD + 状态机辅助查询
- 启动期恢复：把 `status='running'` 的孤儿任务（应用上次崩溃时未走完）标记为 `pending`，等待用户手动重提交
- Tauri commands：`submit_task` / `list_tasks` / `get_task` / `cancel_task`
- Tauri event：`task-status-changed`，payload 通过 tauri-specta `collect_events!` 自动导出 TS 类型
- StubProvider：仅测试用，`submit` 立刻返回 task_id，`poll` 按时间推进 Pending → Running → Success；`download` 写一个固定 PNG 字节
- 端到端单元测试：注册 stub → 提交 → 轮询直到 success → 验证 DB 行 + event 已发送

**非目标**（明确划归后续 spec）：

- 真实 provider 实现 → spec-17（百炼 wan2.7 + happyhorse）
- 本地素材 → 公网 URL 适配 → spec-16（OSS uploader）
- 任务结果下载到 `assets/` + 自动建 Asset 行 → 由 spec-17 在 download 后衔接（本 spec 仅暴露 `download(dest)` API）
- 任务提交表单 / 任务面板 / 结果预览 UI → spec-18
- CLI `mango task ...` → spec-19
- 高级重试策略（指数退避、最大次数）—— MS2 范围内 trait 只暴露一次性 submit；失败任务由用户在 UI 重新提交
- 任务取消的强制中断——`cancel` 调 `provider.cancel`（best-effort）+ 置位 status；下一轮 tick 时轮询协程检测 cancelled 即退出
- 进度百分比的精确推送——只支持 provider 自报 progress，无则 None；UI 显示「生成中...」即可

## 技术方案

### 目录/文件结构

```text
crates/core/src/
├── lib.rs                              # +pub mod provider; +pub mod task_engine;
├── error.rs                            # +Provider(String); +TaskEngine(String); +Cancelled
├── startup.rs                          # +reset_orphan_running_tasks(conn)
├── models/
│   ├── mod.rs                          # +pub mod generation_task;
│   └── generation_task.rs              # 新增：GenerationTask + GenerationTaskStatus + CreateGenerationTaskInput
├── db/queries/
│   ├── mod.rs                          # +pub mod generation_task;
│   └── generation_task.rs              # 新增：CRUD + transition_status + reset_orphans
├── provider/                           # 新增模块
│   ├── mod.rs                          # 重导出 traits + registry + stub
│   ├── traits.rs                       # ModelProvider trait + GenerationParams + ProviderTaskStatus
│   ├── registry.rs                     # ProviderRegistry: HashMap<ProviderId, Arc<dyn ModelProvider>>
│   └── stub.rs                         # StubProvider（仅测试 + dev）
└── task_engine/                        # 新增模块
    ├── mod.rs                          # 重导出 TaskEngineHandle, TaskEvent, spawn
    ├── handle.rs                       # TaskEngineHandle: submit/cancel/list_running 的对外 API
    ├── runner.rs                       # 单任务的轮询循环
    ├── state.rs                        # 状态机：合法跃迁 + DB 持久化封装
    └── events.rs                       # TaskEvent 内部 channel 类型 + 序列化辅助

src-tauri/src/
├── state.rs                            # AppState 加 task_engine: Arc<TaskEngineHandle>
├── events.rs                           # 新增：TaskStatusChanged event 定义（tauri-specta）
├── lib.rs                              # setup 中构造 TaskEngine、把 channel 接到 emit
└── commands/
    ├── mod.rs                          # +pub mod task;
    └── task.rs                         # submit_task / list_tasks / get_task / cancel_task

src/
├── lib/bindings/                       # tauri-specta 自动重新生成
├── hooks/
│   ├── useTaskList.ts                  # 占位：基础 TanStack Query 包装（具体页面 spec-18）
│   ├── useSubmitTask.ts                # 占位
│   └── useTaskStatusListener.ts        # 监听 task-status-changed → invalidate task list query
└── lib/queryKeys.ts                    # +tasks(projectId?) query key
```

### 依赖项变更

**Rust（无新增）**——`async-trait`、`tokio`、`tokio-rusqlite`、`uuid`、`serde`、`tracing`、`thiserror` MS1 已引入。

**Tauri 插件（无新增）**——event 系统是 Tauri 核心。

**npm（无新增）**——TanStack Query / Zustand 已在。

> ⚠️ event payload 类型导出走 **tauri-specta 的 `collect_events!`**，与 commands 同一套；不引入 `ts-rs`。`docs/技术选型.md` §2.1 中 `ts-rs 12.x` 的提法保留为后续逃生路（如果某些非 event 非 command 类型需独立导出）。

### 数据模型

`generation_task` 表已存在（`001_initial.sql`），列包含 `id / shot_id / provider_id / model_id / account_id / task_type / params_json / status / result_asset_id / external_task_id / started_at / finished_at / error_message / retry_count / created_at`。**本 spec 不加 migration**。

`crates/core/src/models/generation_task.rs`：

```rust
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GenerationTaskStatus {
    Pending,
    Running,
    Success,
    Failed,
    Cancelled,
}

impl GenerationTaskStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Success | Self::Failed | Self::Cancelled)
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskKind {
    Text,
    Image,
    Video,
    Audio,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GenerationTask {
    pub id: String,
    pub shot_id: Option<String>,
    pub provider_id: String,
    pub model_id: String,
    pub account_id: String,
    pub task_type: TaskKind,
    pub params_json: String,
    pub status: GenerationTaskStatus,
    pub result_asset_id: Option<String>,
    pub external_task_id: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub error_message: Option<String>,
    #[specta(type = specta_typescript::Number)]
    pub retry_count: i64,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Type)]
pub struct CreateGenerationTaskInput {
    #[serde(default)]
    pub shot_id: Option<String>,
    pub provider_id: String,
    pub model_id: String,
    pub account_id: String,
    pub task_type: TaskKind,
    /// JSON-encoded provider-specific parameters. Validated by the chosen
    /// provider, not this layer.
    #[serde(default)]
    pub params_json: Option<String>,
}
```

> **设计说明**：`params_json` 不在 core 反序列化为强类型——每个 provider 的参数 schema 不同（wan2.7 要 `size` 字段、happyhorse 要 `media[]` 字段），交给 provider 自己解析。core 只确保是合法 JSON。

### Rust 错误类型扩展

`crates/core/src/error.rs`：

```rust
#[derive(Error, Debug)]
pub enum CoreError {
    // ... existing variants ...

    /// 包装 provider 实现层抛出的所有错误（HTTP/认证/参数/远端业务错）。
    /// 用 String 是为了避免 trait object 方差问题——具体 provider 实现内部
    /// 用 `thiserror`，到 core 边界统一转 String。
    #[error("provider error: {0}")]
    Provider(String),

    /// 任务引擎自身错误（registry miss、状态非法跃迁等）。
    #[error("task engine error: {0}")]
    TaskEngine(String),

    /// 任务被取消（区别于 Failed，UI 不展示为错误）。
    #[error("task cancelled")]
    Cancelled,
}
```

### ModelProvider trait

`crates/core/src/provider/traits.rs`：

```rust
use std::path::{Path, PathBuf};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::Result;

/// 提交参数。`provider_params` 是 provider 自定义 JSON。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GenerationParams {
    pub model_id: String,
    pub prompt: String,
    /// 解码后的 provider 私有参数（如 wan2.7 的 size、happyhorse 的 media[]）。
    /// 由 caller（task_engine）从 generation_task.params_json 反序列化得到。
    pub provider_params: serde_json::Value,
    /// 凭证：API key 等已从 keyring 取出的明文，仅在内存中流转。
    pub credentials: ProviderCredentials,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProviderCredentials {
    pub api_key: String,
    /// 由 spec-16 扩展：附加配置如 OSS endpoint/bucket，序列化进来。
    /// MS1 不写、留空 None；spec-16 写入。
    #[serde(default)]
    pub extra_json: Option<String>,
}

/// Provider 视角的任务状态。core 任务引擎再按规则映射到 `GenerationTaskStatus`。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderTaskStatus {
    Pending,
    Running { progress: Option<u8> },
    Success { result_url: String },
    Failed { message: String },
}

#[async_trait]
pub trait ModelProvider: Send + Sync + 'static {
    /// 提交任务，返回 provider 侧任务 ID。同步模型在内部用一次性 future 包装
    /// REST 响应（详见 spec-17 wan2.7 同步包装）。
    async fn submit(&self, params: GenerationParams) -> Result<String>;

    /// 查询状态。同步模型直接命中内部缓存返回 Success；异步模型走 REST。
    async fn poll(&self, external_task_id: &str) -> Result<ProviderTaskStatus>;

    /// 取消。best-effort——provider 不支持时返回 Ok 即可。
    async fn cancel(&self, external_task_id: &str) -> Result<()>;

    /// 把结果下载到 dest（task_engine 决定 dest 路径在 `<project_root>/assets/`
    /// 下；本 spec 不强制）。返回实际写入路径（可能因 provider 给的扩展名补全）。
    async fn download(&self, external_task_id: &str, dest: &Path) -> Result<PathBuf>;
}
```

### Provider 注册表

`crates/core/src/provider/registry.rs`：

```rust
use std::collections::HashMap;
use std::sync::Arc;

use crate::error::{CoreError, Result};
use super::traits::ModelProvider;

#[derive(Default, Clone)]
pub struct ProviderRegistry {
    inner: Arc<HashMap<String, Arc<dyn ModelProvider>>>,
}

impl ProviderRegistry {
    pub fn builder() -> ProviderRegistryBuilder { ProviderRegistryBuilder::default() }

    pub fn get(&self, provider_id: &str) -> Result<Arc<dyn ModelProvider>> {
        self.inner.get(provider_id).cloned().ok_or_else(|| {
            CoreError::TaskEngine(format!("provider not registered: {provider_id}"))
        })
    }
}

#[derive(Default)]
pub struct ProviderRegistryBuilder {
    map: HashMap<String, Arc<dyn ModelProvider>>,
}

impl ProviderRegistryBuilder {
    pub fn register(mut self, id: impl Into<String>, p: Arc<dyn ModelProvider>) -> Self {
        self.map.insert(id.into(), p);
        self
    }
    pub fn build(self) -> ProviderRegistry {
        ProviderRegistry { inner: Arc::new(self.map) }
    }
}
```

> **设计说明**：注册表用 `Arc<HashMap>` 而不是 `RwLock<HashMap>`——MS2 范围内 provider 集合在启动时构造完毕、运行期不变；不需要支持热增删。spec-17 提交时往 builder 里 `.register("bailian", Arc::new(BailianProvider::new(...)))` 即可。

### StubProvider（仅测试 / dev）

`crates/core/src/provider/stub.rs`：

```rust
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use crate::error::Result;
use super::traits::*;

/// 用于单元测试与开发期 UI 联调。提交后立即返回 task id；前 2 次 poll 报
/// Running，第 3 次报 Success。download 写入一个 1×1 PNG 字节。
#[derive(Default)]
pub struct StubProvider {
    polls: Arc<AtomicU8>,
}

#[async_trait]
impl ModelProvider for StubProvider {
    async fn submit(&self, _params: GenerationParams) -> Result<String> {
        Ok(format!("stub-{}", uuid::Uuid::new_v4()))
    }

    async fn poll(&self, _external_task_id: &str) -> Result<ProviderTaskStatus> {
        let n = self.polls.fetch_add(1, Ordering::SeqCst);
        if n < 2 {
            Ok(ProviderTaskStatus::Running { progress: Some((n + 1) * 30) })
        } else {
            Ok(ProviderTaskStatus::Success { result_url: "stub://ok".into() })
        }
    }

    async fn cancel(&self, _external_task_id: &str) -> Result<()> { Ok(()) }

    async fn download(&self, _external_task_id: &str, dest: &Path) -> Result<PathBuf> {
        // 1×1 transparent PNG 共 67 字节
        const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1f\x15\xc4\x89\x00\x00\x00\x0dIDATx\x9cc\xf8\xff\xff?\x00\x05\xfe\x02\xfe\xdcMb\xa3\x00\x00\x00\x00IEND\xaeB`\x82";
        std::fs::write(dest, PNG)?;
        Ok(dest.to_path_buf())
    }
}
```

### 任务引擎结构

`crates/core/src/task_engine/handle.rs`：

```rust
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tokio_rusqlite::Connection;

use crate::error::Result;
use crate::models::generation_task::{GenerationTask, GenerationTaskStatus, CreateGenerationTaskInput};
use crate::provider::registry::ProviderRegistry;
use super::events::TaskEvent;

/// 引擎句柄。所有对外 API 都走这一处，runner 协程是内部细节。
#[derive(Clone)]
pub struct TaskEngineHandle {
    db: Connection,
    providers: ProviderRegistry,
    semaphore: Arc<Semaphore>,
    event_tx: mpsc::UnboundedSender<TaskEvent>,
}

impl TaskEngineHandle {
    /// 启动引擎。返回 handle 和事件 receiver（src-tauri 的 setup 把 receiver 接到
    /// AppHandle.emit，CLI 可以丢弃）。
    pub fn spawn(
        db: Connection,
        providers: ProviderRegistry,
        max_concurrency: usize,
    ) -> (Self, mpsc::UnboundedReceiver<TaskEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let handle = Self {
            db,
            providers,
            semaphore: Arc::new(Semaphore::new(max_concurrency)),
            event_tx: tx,
        };
        (handle, rx)
    }

    /// 提交任务：写 DB（status='pending'）→ spawn runner 协程 → 返回 task id。
    /// 不阻塞——runner 内部抢信号量，确保并发上限。
    pub async fn submit(&self, input: CreateGenerationTaskInput) -> Result<String> {
        // 1. INSERT row with status='pending'
        // 2. tokio::spawn(super::runner::run(self.clone(), task_id))
        // 3. 立即返回 task_id
        unimplemented!("see runner.rs")
    }

    /// 取消：UPDATE status='cancelled'（仅限当前 pending/running 时）+ 调
    /// provider.cancel（best-effort，错误吞掉）。runner 下一轮 poll 检测到
    /// status=cancelled 后退出。
    pub async fn cancel(&self, task_id: &str) -> Result<()> {
        unimplemented!()
    }

    pub async fn list(&self, filter: ListFilter) -> Result<Vec<GenerationTask>> { unimplemented!() }
    pub async fn get(&self, task_id: &str) -> Result<GenerationTask> { unimplemented!() }
}

#[derive(Debug, Default, Clone)]
pub struct ListFilter {
    pub project_id: Option<String>,
    pub status: Option<GenerationTaskStatus>,
    pub limit: Option<u32>,
}
```

### 单任务 runner

`crates/core/src/task_engine/runner.rs`（核心循环，伪代码骨架）：

```rust
use std::time::Duration;
use tokio::time::sleep;

use crate::error::{CoreError, Result};
use crate::models::generation_task::GenerationTaskStatus;
use crate::provider::traits::{GenerationParams, ProviderTaskStatus};

use super::handle::TaskEngineHandle;
use super::events::TaskEvent;
use super::state;

const POLL_INTERVAL: Duration = Duration::from_secs(5);

pub async fn run(engine: TaskEngineHandle, task_id: String) {
    // 抢并发信号量；释放在 drop 时
    let _permit = engine.semaphore.clone().acquire_owned().await.unwrap();

    // 1. 加载 task 行 + 解析 provider_id / model_id / params_json
    let task = match state::load(&engine.db, &task_id).await { /* ... */ };
    let provider = match engine.providers.get(&task.provider_id) { /* ... */ };

    // 2. 解析 account_id → 从 keyring 取 api_key（走 account::service 里已有的 fn）
    //    spec-15 不实现这一步细节——以下伪：
    let credentials = state::load_credentials(&engine.db, &task.account_id).await;

    // 3. 转 status='running'，落库 + emit Running 事件
    state::transition(&engine.db, &task_id, GenerationTaskStatus::Running, None, None).await?;
    engine.event_tx.send(TaskEvent::status(&task_id, GenerationTaskStatus::Running, None));

    // 4. submit
    let params = GenerationParams { /* 从 task.params_json 解 */ };
    let external_id = match provider.submit(params).await {
        Ok(id) => id,
        Err(e) => return state::fail(&engine, &task_id, &e.to_string()).await,
    };
    state::set_external_id(&engine.db, &task_id, &external_id).await?;

    // 5. 轮询直到终态或被取消
    loop {
        sleep(POLL_INTERVAL).await;
        if state::is_cancelled(&engine.db, &task_id).await? {
            let _ = provider.cancel(&external_id).await;
            return; // status 已是 cancelled，不再 transition
        }
        match provider.poll(&external_id).await {
            Ok(ProviderTaskStatus::Pending | ProviderTaskStatus::Running { .. }) => continue,
            Ok(ProviderTaskStatus::Success { result_url }) => {
                // download 由后续 spec-17 在外层包装（写入 assets/、建 Asset 行）。
                // 本 spec 仅置 status=success + emit；不下载。
                state::succeed(&engine, &task_id, &result_url).await;
                return;
            }
            Ok(ProviderTaskStatus::Failed { message }) => {
                return state::fail(&engine, &task_id, &message).await;
            }
            Err(e) => return state::fail(&engine, &task_id, &e.to_string()).await,
        }
    }
}
```

> **设计说明**：本 spec 的 runner 在 success 分支**只落 status / 留 external_task_id**，不下载结果。spec-17 会在 BailianProvider 外层包一层「BailianTaskWrapper」，在 success 时调 `provider.download(&external_id, dest)` 把结果写到 `<project_root>/assets/{asset_id}.<ext>` 并建 Asset 行——这是 spec-17 范围。本 spec 的 runner 保持「provider 中立」。

### 状态机辅助（`task_engine/state.rs`）

封装合法跃迁 + DB 持久化 + 事件发送，runner 用它做底层操作。关键约束：

- `pending → running`、`running → success/failed/cancelled`、`pending → cancelled` 合法
- 任何到达终态后再 transition 报 `CoreError::TaskEngine("invalid transition")`
- 每次 transition 在同一事务里 UPDATE 行 + `started_at` / `finished_at` 自动填充
- transition 成功后**立即** `event_tx.send(...)`；失败不 emit

### 启动期任务恢复

`crates/core/src/db/queries/generation_task.rs` 添加：

```rust
/// 把所有 status='running' 的行重置为 'pending'，error_message 标注「应用上次未正常退出」。
/// 在 startup::initialize 第三步调用。
pub fn reset_orphan_running(conn: &Connection) -> Result<usize> {
    let n = conn.execute(
        "UPDATE generation_task
         SET status = 'pending',
             error_message = COALESCE(error_message, '') || '[orphan reset on startup]',
             retry_count = retry_count + 1
         WHERE status = 'running'",
        [],
    )?;
    Ok(n)
}
```

`crates/core/src/startup.rs` `initialize` 在 `apply_seed` 之后追加：

```rust
let n = crate::db::queries::generation_task::reset_orphan_running(conn)?;
if n > 0 {
    tracing::warn!("reset {n} orphan running task(s) to pending after restart");
}
```

> **设计说明**：选「重置为 pending」而不是「保持 running 让 runner 续跑」。理由：(a) 应用重启后内存里没有 runner 协程，状态不一致；(b) 让用户主动决定是否重提（在 UI 看到 pending 任务可手动 retry 或 cancel）；(c) `retry_count + 1` 给后续可能的「自动重试上限」留出钩子。**不删除外部 task_id**——provider 那侧任务可能还在跑，删了就找不回来了。

### Tauri event 推送

`src-tauri/src/events.rs`（新文件）：

```rust
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;

#[derive(Clone, Serialize, Deserialize, Type, Event)]
pub struct TaskStatusChanged {
    pub task_id: String,
    pub status: mango_core::models::generation_task::GenerationTaskStatus,
    /// 0..=100；None 表示未知。
    #[specta(type = Option<specta_typescript::Number>)]
    pub progress: Option<u8>,
    pub error_message: Option<String>,
}
```

`src-tauri/src/lib.rs` `make_builder()` 新增：

```rust
.events(tauri_specta::collect_events![events::TaskStatusChanged])
```

`setup` 中（在 `manage(AppState)` 之前）启动一个 forwarder：

```rust
let providers = mango_core::provider::registry::ProviderRegistry::builder()
    // spec-17 在这里 .register("bailian", Arc::new(BailianProvider::new(...)))
    .build();
let (engine, mut event_rx) = mango_core::task_engine::TaskEngineHandle::spawn(
    db.clone(), providers, 4 /* max concurrency */
);
let app_handle = app.handle().clone();
tauri::async_runtime::spawn(async move {
    use mango_core::task_engine::events::TaskEvent;
    while let Some(ev) = event_rx.recv().await {
        match ev {
            TaskEvent::StatusChanged { task_id, status, progress, error_message } => {
                let _ = events::TaskStatusChanged { task_id, status, progress, error_message }
                    .emit(&app_handle);
            }
        }
    }
});
```

`AppState` 新增 `task_engine: Arc<TaskEngineHandle>`，commands 通过 `State<AppState>` 访问。

### Tauri commands

`src-tauri/src/commands/task.rs`（新文件，签名）：

```rust
use mango_core::models::generation_task::*;
use tauri::State;
use std::sync::Arc;

use crate::error::IpcError;
use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub async fn submit_task(
    state: State<'_, AppState>,
    input: CreateGenerationTaskInput,
) -> Result<String, IpcError> {
    state.task_engine.submit(input).await.map_err(IpcError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_task(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<(), IpcError> { state.task_engine.cancel(&task_id).await.map_err(IpcError::from) }

#[tauri::command]
#[specta::specta]
pub async fn get_task(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<GenerationTask, IpcError> { state.task_engine.get(&task_id).await.map_err(IpcError::from) }

#[tauri::command]
#[specta::specta]
pub async fn list_tasks(
    state: State<'_, AppState>,
    project_id: Option<String>,
    status: Option<GenerationTaskStatus>,
    limit: Option<u32>,
) -> Result<Vec<GenerationTask>, IpcError> {
    state.task_engine
        .list(mango_core::task_engine::ListFilter { project_id, status, limit })
        .await
        .map_err(IpcError::from)
}
```

注册到 `make_builder()`：

```rust
commands::task::submit_task,
commands::task::cancel_task,
commands::task::get_task,
commands::task::list_tasks,
```

### 前端 hooks（占位）

`src/hooks/useTaskList.ts`：

```ts
import { useQuery } from "@tanstack/react-query";
import { commands } from "@/lib/bindings/commands";
import { taskKeys } from "@/lib/queryKeys";

export function useTaskList(projectId?: string, status?: string) {
  return useQuery({
    queryKey: taskKeys.list(projectId, status),
    queryFn: () => commands.listTasks(projectId ?? null, status ?? null, null),
  });
}
```

`src/hooks/useTaskStatusListener.ts`（在 App.tsx 顶层挂一次）：

```ts
import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { events } from "@/lib/bindings/commands";
import { taskKeys } from "@/lib/queryKeys";

export function useTaskStatusListener() {
  const qc = useQueryClient();
  useEffect(() => {
    const unlisten = events.taskStatusChanged.listen(() => {
      qc.invalidateQueries({ queryKey: taskKeys.all });
    });
    return () => { unlisten.then((fn) => fn()); };
  }, [qc]);
}
```

`src/lib/queryKeys.ts` 新增：

```ts
export const taskKeys = {
  all: ["tasks"] as const,
  list: (projectId?: string, status?: string) => ["tasks", "list", projectId, status] as const,
  detail: (taskId: string) => ["tasks", "detail", taskId] as const,
};
```

> **设计说明**：本 spec 仅暴露最小 hook 集——具体表单、面板、预览组件由 spec-18 实现。`useTaskStatusListener` 走「event 触发 invalidate query」模式，而不是直接更新缓存——`invalidate` 简单且容错（broker 偶尔丢消息也能自愈）。

### CLI

CLI 的 task 子命令由 spec-19 实现。本 spec 的 core API 必须能直接被 CLI 调用——`TaskEngineHandle::spawn` 不依赖任何 Tauri 类型即满足。

### 错误场景

| 场景 | 行为 |
|---|---|
| `submit` 时 provider_id 在 registry 里没注册 | 立即返回 `CoreError::TaskEngine("provider not registered: ...")`，不写 DB |
| `submit` 时 model_id / account_id 在 DB 不存在 | FK 约束触发；返回 `CoreError::Sqlite` |
| `submit` 时 params_json 不是合法 JSON | 在 core 入口校验：`serde_json::from_str(...)` 失败 → `CoreError::Validation` |
| runner 启动后 provider.submit 报错 | status → failed，error_message 写错误，event 发送 Failed |
| runner 轮询过程中应用退出 | 协程被 tokio runtime drop；DB 行卡在 running；下次启动 `reset_orphan_running` 转回 pending |
| 用户 cancel 一个 pending 任务 | runner 检测到 status=cancelled 后直接退出，不调 provider.cancel（外部任务还没建） |
| 用户 cancel 一个 running 任务 | UPDATE status='cancelled' + 调 provider.cancel（best-effort） |
| 用户 cancel 一个已 success 的任务 | `transition` 拒绝（终态不可变）；返回 `Validation("task already terminal")` |
| event channel 满（mpsc 不会满，unbounded） | 不会发生——本 spec 用 unbounded |
| forwarder 协程 panic | 日志 error；后续状态变更收不到 event；UI 表现为「任务不刷新」——MS2 范围内不做自动重启 |

## 测试策略

### Rust 单元测试

`crates/core/src/models/generation_task.rs`：
- `GenerationTaskStatus::is_terminal` 各分支正确
- `as_str` 与 DB 写入字符串完全一致

`crates/core/src/db/queries/generation_task.rs`：
- create / get / list 基础 CRUD
- `reset_orphan_running` 把 running 行转 pending、retry_count + 1、error_message 追加 marker；其他状态行不变
- `transition_status`：合法跃迁通过，非法跃迁返回 Err
- `list` 按 project_id（join shot/episode）、status、limit 三个过滤维度分别测

`crates/core/src/provider/registry.rs`：
- builder 注册后 get 命中
- get 未注册时返回 TaskEngine 错误

`crates/core/src/provider/stub.rs`：
- 三次 poll 序列：Running(30%) → Running(60%) → Success
- download 写入文件大小 > 0

### Rust 集成测试

`crates/core/tests/task_engine_e2e.rs`（新文件）：

```rust
// 端到端：注册 stub provider → 提交任务 → 用 timeout 等 event 到 Success
// → 验证 DB 行 status=success、external_task_id 非空
// 用 :memory: + tokio::test
```

`crates/core/tests/task_engine_cancel.rs`：
- 提交一个 stub 任务、立即 cancel、验证 5s 内进入 cancelled、retry_count 不变

`crates/core/tests/task_engine_orphan_recovery.rs`：
- 直接 INSERT 一个 status='running' 行 → 调 `startup::initialize` → 验证转 pending、retry_count + 1

### 前端测试

`src/hooks/useTaskStatusListener.test.tsx`：
- mock `events.taskStatusChanged.listen` → 触发 → 验证 queryClient.invalidateQueries 被调用一次

具体 UI 测试由 spec-18 覆盖。

### 手动验证清单

- [ ] `pnpm tauri dev` 启动后无控制台错误
- [ ] dev 模式下临时把 stub 注册到 registry（用 `cfg(debug_assertions)` 包），手动 invoke `submit_task` IPC（用浏览器 DevTools console），观察 status → pending → running → success
- [ ] 在 running 中关闭应用 → 重启 → 看 log 「reset N orphan running task(s)」
- [ ] DB 直查 `SELECT * FROM generation_task ORDER BY created_at DESC LIMIT 5` 验证状态/时间字段填充正确

## 验收标准

- [ ] `crates/core/src/lib.rs` 新增 `pub mod provider; pub mod task_engine;`
- [ ] `ModelProvider` trait 定义完整（4 个方法）+ `GenerationParams` + `ProviderCredentials` + `ProviderTaskStatus`
- [ ] `ProviderRegistry::builder().register(...).build()` 流式 API 可用
- [ ] `StubProvider` 在 `cargo test` 中通过 3 个轮询步骤
- [ ] `TaskEngineHandle::spawn` 返回 handle + event receiver
- [ ] `submit` / `cancel` / `get` / `list` 四个 API 全部实现
- [ ] `generation_task` 表 CRUD + `reset_orphan_running` + `transition_status` 全套查询
- [ ] `startup::initialize` 增加 reset_orphan_running 步骤；测试覆盖
- [ ] `TaskStatusChanged` event 通过 tauri-specta `collect_events!` 导出，前端 `events.taskStatusChanged.listen` 可调用
- [ ] 4 个 Tauri commands 注册到 `make_builder()` collect_commands! 列表
- [ ] AppState 新增 `task_engine: Arc<TaskEngineHandle>`
- [ ] forwarder 协程在 setup 中启动，把 mpsc → AppHandle.emit
- [ ] `useTaskList` / `useTaskStatusListener` / `useSubmitTask` 三 hook 占位实现
- [ ] `cargo test --workspace` 全绿
- [ ] `cargo clippy` 无 warning
- [ ] `pnpm typecheck` 全绿；前端 hook 调用编译通过
- [ ] `cargo test -p mango-tauri export_bindings` 重新生成 commands.ts 与 events.ts，diff 干净

## 依赖关系

- **前置**：MS1 全部 spec（spec-10~14）；特别依赖 `account::service` 里 keyring 取 API key 的现成 API（spec-13 已实现）
- **平行**：无
- **后续**：
  - SPEC-16（OSS uploader + 账号扩展）会在 `ProviderCredentials.extra_json` 上写入 OSS 配置
  - SPEC-17（百炼 provider）实现 `ModelProvider` trait 并通过 registry 注册；同时实现「下载结果 → Asset 入库」的外层包装
  - SPEC-18（任务 UI）消费 hooks + commands
  - SPEC-19（CLI 任务命令）直接调用 `TaskEngineHandle` API

## 与原任务文档的差异

`docs/iterations/ms2-AI生图生视频/开发任务.md` 的任务 1（ModelProvider trait 定义）+ 任务 3（任务引擎基础）+ 任务 5（Tauri event 进度推送）整合为本 spec。原任务 4（结果下载 + 自动创建 Asset）拆出去由 spec-17 在 provider 层处理（避免任务引擎与 Asset 绑死，保持 provider 中立）；原任务 10（基本错误处理）拆为：

- provider 层错误（API 限流、认证失败、余额不足）→ spec-17
- core 层错误（registry miss、参数非法、状态机非法跃迁）→ 本 spec
- UI 错误展示（Toast / 卡片错误信息）→ spec-18
