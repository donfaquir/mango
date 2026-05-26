# Spec 19 — CLI 任务命令

> 对应 MS2 任务 9。在 `mango` CLI 下加 `task` 子命令家族：submit / status / list / cancel。
>
> CLI 与 GUI 共享同一个 SQLite（`<app_data_dir>/com.mango.app/mango.db`）和 keyring，因此 CLI 提交的任务在 GUI 任务面板里直接可见，反之亦然。

依赖：spec-15（task_engine + task_service）+ spec-16（账号/Provider/keyring）+ spec-17（百炼 provider 注册）

非目标：
- CLI 不提供「批量提交从 CSV 读取」（MS4 才做）
- CLI 不复制 GUI 的「采用结果」流程（MS3 才做）
- CLI 不提供本地结果文件预览（不是 CLI 的职责，告知路径即可）

---

## 1. CLI 入口扩展

### 1.1 `crates/cli/src/main.rs`

`Commands` 枚举追加一项：

```rust
#[derive(clap::Subcommand)]
enum Commands {
    Project(commands::project::ProjectArgs),
    Character(commands::character::CharacterArgs),
    Account(commands::account::AccountArgs),
    /// AI generation tasks (submit / poll / list / cancel)
    Task(commands::task::TaskArgs),  // NEW
}
```

`run()` 内 dispatch：

```rust
Commands::Task(args) => commands::task::execute(&conn, &app_data_dir, args)?,
```

`task::execute` 接 `app_data_dir` 是为了拿项目根路径（spec-15 的 task_engine 需要 project_root 才能下载结果到 assets/）。

### 1.2 同步 vs 异步

CLI 用的是 `rusqlite::Connection`（sync）。task_engine 内部依赖 `tokio_rusqlite::Connection`（async wrapper）。两条路：

1. CLI 也起一个 tokio runtime，把 `rusqlite::Connection` 包装成 `tokio_rusqlite::Connection`（实际就是 `Connection::from_handle`）
2. 在 task_service 暴露一组 sync API（封住 runtime 内部）

**采用方案 1**。理由：CLI 已经在 main.rs 里持有 sync connection 是历史包袱，但一旦进入 `task` 命令我们就明确启动 tokio runtime，其他命令仍走 sync。**约束**：CLI 启动时如果 `Commands::Task(_)`，**重新打开**一个 `tokio_rusqlite::Connection`（指向同一文件），不复用顶层 sync conn —— 避免 sync/async 在同一 connection 上交叉调用。

```rust
// crates/cli/src/commands/task.rs (top of execute)
pub fn execute(_conn: &Connection, app_data_dir: &Path, args: TaskArgs) -> anyhow::Result<()> {
    // Ignore the parent's sync conn entirely; task engine needs its own async one.
    let db_path = mango_core::startup::default_db_path(app_data_dir);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let conn = mango_core::db::open_async(&db_path).await?;
        // ...
    })
}
```

> spec-15 应该已暴露 `mango_core::db::open_async(path) -> tokio_rusqlite::Connection` 与 `mango_core::startup::default_db_path(app_data_dir)`。如果未暴露，spec-19 实施时要在 core crate 补这两个工具。

---

## 2. 子命令清单

```
mango task submit <args...>     # 提交一个新任务
mango task status <task_id>     # 显示单个任务详情
mango task list [filters...]    # 列出任务（默认显示当前一周）
mango task cancel <task_id>     # 取消任务
```

显式 **不** 实现 `task retry` —— CLI 重试 = 重新执行一次 submit；retry 只在 GUI 有意义（沿用上次表单值）。

### 2.1 `mango task submit`

```text
USAGE:
    mango task submit <COMMON_OPTIONS> [MODEL_OPTIONS] --prompt <PROMPT>

COMMON OPTIONS:
    --project-id <ID>        必填，目标项目 ID
    --provider <ID>          provider id（默认 bailian）。会自动选该 provider 的第一个 account
    --account-id <ID>        指定具体 account ID（覆盖 --provider 的自动选择）
    --model <ID>             必填，模型 ID（如 wan2.7-image-pro / happyhorse-1.0-r2v）
    --type <image|video>     可省略，工具内据 model 反推（百炼模型已知映射）；不一致时报错
    --prompt <TEXT>          必填；多行用 shell 引号
    --prompt-file <PATH>     与 --prompt 二选一，从文件读取
    --wait                   阻塞跟随到任务终态，stderr 输出进度，stdout 仅最后输出 task_id 和结果路径
    --json                   输出 JSON（替代默认人类可读输出）

MODEL-SPECIFIC OPTIONS (wan2.7-image-pro):
    --n <1..=4>              生成张数，默认 1
    --size <1K|2K|4K>        默认 2K
    --enable-sequential       组图保持一致
    --negative-prompt <TXT>

MODEL-SPECIFIC OPTIONS (happyhorse-1.0-r2v):
    --subject <ID>           可重复，至少一次
    --resolution <720P|1080P>  默认 720P
    --ratio <16:9|9:16|1:1>    默认 16:9
    --duration <3|5|10>        默认 5
```

例子：

```bash
# 文生图
mango task submit \
  --project-id 9c2... \
  --provider bailian \
  --model wan2.7-image-pro \
  --prompt "教室阳光照在书桌上的少年" \
  --n 2 --size 2K

# 参考图生视频
mango task submit \
  --project-id 9c2... \
  --provider bailian \
  --model happyhorse-1.0-r2v \
  --subject ab12-character-id \
  --prompt "小明翻开书本，抬头微笑" \
  --duration 5 \
  --wait
```

输出（无 `--wait`，默认）：

```text
Submitted
  ID:        7f3a1c2e-...
  Type:      image
  Status:    pending
  Provider:  bailian / wan2.7-image-pro
  Account:   ****a1b2 (默认)

Run `mango task status 7f3a1c2e --watch` to follow progress.
```

输出（带 `--wait`，stderr 有进度，stdout 是结果）：

```text
[stderr]
↻ pending → running ...
↻ running 30%
↻ running 70%
✓ success in 38s

[stdout]
7f3a1c2e-...  /Users/.../projects/<id>/assets/abcd1234.png
```

`--wait` + `--json` 时 stdout 输出：

```json
{
  "task_id": "7f3a...",
  "status": "success",
  "duration_ms": 38214,
  "result_asset_path": "assets/abcd1234.png",
  "result_asset_id": "abcd1234-...",
  "model_id": "wan2.7-image-pro",
  "task_type": "image"
}
```

### 2.2 `mango task status <id> [--watch] [--json]`

```text
USAGE:
    mango task status <TASK_ID> [--watch] [--json]

OPTIONS:
    --watch  阻塞循环（poll 间隔 5s）直到任务终态
    --json   JSON 输出
```

输出：

```text
Task 7f3a1c2e-...
  Status:        running (45%)
  Type:          image
  Provider:      bailian / wan2.7-image-pro
  Account:       ****a1b2 (default)
  Project:       9c2... (mango-demo)
  Submitted:     2026-05-25 14:32:11
  Started:       2026-05-25 14:32:13
  Last update:   2026-05-25 14:32:48
  Prompt:        教室阳光照在书桌上的少年...
```

完成后追加：

```text
  Finished:      2026-05-25 14:32:51
  Result:        assets/abcd1234.png
                 (/Users/.../projects/9c2.../assets/abcd1234.png)
```

失败时：

```text
  Failed at:     2026-05-25 14:35:01
  Error:         RateLimited: too many concurrent tasks (DashScope 429)
```

### 2.3 `mango task list`

```text
USAGE:
    mango task list [OPTIONS]

OPTIONS:
    --project-id <ID>          只显示某项目下的任务
    --status <STATUS>          可重复：pending|running|success|failed|cancelled
    --provider <ID>
    --since <DATE_OR_DURATION> 默认 7d；支持 ISO 日期或 1d/12h/30m
    --limit <N>                默认 50，最大 500
    --json                     输出 JSON 数组（一行一对象，便于 jq）
```

人类可读：

```text
ID         PROJECT     MODEL                STATUS    SUBMITTED            ELAPSED
7f3a1c2e   mango-demo  wan2.7-image-pro     success   2026-05-25 14:32:11  38s
8b1d4567   mango-demo  happyhorse-1.0-r2v   running   2026-05-25 14:35:00  2m12s
9c5e9e10   demo-vid    happyhorse-1.0-r2v   failed    2026-05-25 14:31:00  21s

3 task(s) — page 1/1
```

`--json` 模式 stdout 一行一个 JSON 对象（NDJSON），方便 `mango task list --json | jq`。

### 2.4 `mango task cancel <id>`

```text
USAGE:
    mango task cancel <TASK_ID> [--yes]
```

提示交互式确认（除非 `--yes`），调用 `task_service::cancel_task`，输出：

```text
Cancelled task 7f3a1c2e-...
  Was status:  running (will move to cancelled in a moment)
```

提交后立刻返回；CLI 不等待 cancelled 状态落库。

---

## 3. 实现要点

### 3.1 文件结构

```
crates/cli/src/commands/task.rs   # 单文件 ≤ 300 行
```

如果超过 300 行（实际可能 350+），按下面拆开：

```
crates/cli/src/commands/task/
├── mod.rs        # TaskArgs / TaskAction / execute() dispatch
├── submit.rs     # submit + 模型分发
├── status.rs     # status + watch 循环
├── list.rs       # list + 过滤
├── cancel.rs     # cancel
└── output.rs     # 共用：JSON 序列化、表格 helper、彩色状态符号
```

### 3.2 提交流程伪码

```rust
// commands/task/submit.rs
pub async fn run(conn: &TokioConn, app_data_dir: &Path, args: SubmitArgs) -> anyhow::Result<()> {
    let prompt = resolve_prompt(args.prompt.clone(), args.prompt_file.clone())?;

    // Resolve account: --account-id wins, otherwise first account of --provider.
    let account_id = match args.account_id.as_deref() {
        Some(id) => id.to_string(),
        None => {
            let provider = args.provider.as_deref().unwrap_or("bailian");
            account_queries::list(conn, Some(provider.to_string()))
                .await?
                .first()
                .map(|a| a.id.clone())
                .ok_or_else(|| anyhow!(
                    "no account configured for provider `{provider}`. \
                     Run `mango account add --provider-id {provider} ...` first."
                ))?
        }
    };

    // Build provider_params from model-specific flags.
    let (task_type, params_json) = build_params(&args)?;

    // Hand off to task_service (same code path as Tauri command).
    let task = task_service::submit_task(
        conn,
        app_data_dir,
        SubmitTaskInput {
            project_id: args.project_id,
            account_id,
            model_id: args.model.clone(),
            task_type,
            prompt,
            provider_params: params_json,
        },
    ).await?;

    if args.wait {
        wait_to_terminal(conn, &task.id, args.json).await?;
    } else {
        print_submitted_summary(&task, args.json);
    }
    Ok(())
}

fn build_params(args: &SubmitArgs) -> anyhow::Result<(TaskType, serde_json::Value)> {
    match args.model.as_str() {
        "wan2.7-image-pro" => {
            ensure_no_subject(&args.subject)?;
            let v = json!({
                "n": args.n.unwrap_or(1),
                "size": args.size.clone().unwrap_or_else(|| "2K".into()),
                "enable_sequential": args.enable_sequential,
                "negative_prompt": args.negative_prompt.clone(),
            });
            Ok((TaskType::Image, v))
        }
        "happyhorse-1.0-r2v" => {
            if args.subject.is_empty() {
                bail!("--subject is required for happyhorse-1.0-r2v");
            }
            let media = subject_to_media(conn, &args.subject).await?;
            let v = json!({
                "media": media,
                "resolution": args.resolution.clone().unwrap_or_else(|| "720P".into()),
                "ratio": args.ratio.clone().unwrap_or_else(|| "16:9".into()),
                "duration": args.duration.unwrap_or(5),
            });
            Ok((TaskType::Video, v))
        }
        other => bail!("unknown model id `{other}`. Run `mango task list-models`."),
    }
}
```

### 3.3 进度轮询（`--wait` / `--watch`）

实现要点：
- 不订阅 Tauri event（CLI 没有 Tauri runtime）
- 直接 5s 一次查 DB（`task_service::get_task`），打印状态变化
- 状态没变也不打印（除非进度数字变了）
- terminal 状态（success / failed / cancelled）→ 跳出循环
- Ctrl+C 中断 wait 循环，但不取消任务（CLI 进程退出，任务在后端继续；这个行为在 `--help` 里讲清楚）

```rust
async fn wait_to_terminal(
    conn: &TokioConn,
    task_id: &str,
    json: bool,
) -> anyhow::Result<()> {
    use std::io::IsTerminal;
    let stderr_is_tty = std::io::stderr().is_terminal();
    let mut last_printed: Option<(String, Option<u8>)> = None;
    let started = Instant::now();

    loop {
        let task = task_service::get_task(conn, task_id).await?;
        let key = (task.status.to_string(), task.progress);
        if Some(&key) != last_printed.as_ref() {
            print_progress_line(&task, stderr_is_tty);
            last_printed = Some(key);
        }
        if task.status.is_terminal() {
            print_terminal(&task, started.elapsed(), json);
            return Ok(());
        }
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(5)) => {}
            _ = tokio::signal::ctrl_c() => {
                eprintln!("\nDetached from progress (task continues in background).");
                eprintln!("To cancel: mango task cancel {task_id}");
                return Ok(());
            }
        }
    }
}
```

### 3.4 task_engine 在 CLI 进程内的生命周期

**关键问题**：spec-15 的 `TaskEngineHandle` 是后台 worker，负责把 pending → running → success。GUI 进程内一直跑这个 worker；CLI 进程只在执行 `task submit/wait` 时短暂存在。如果 CLI 提交了任务但没有 `--wait`，进程退出后谁来把它推进？

**两种部署形态**：

1. **GUI + CLI 同时跑**：GUI 进程的 task_engine worker 会捡起 CLI 写入的 pending 任务并执行。理想路径。
2. **只有 CLI（GUI 没开）**：必须自己跑 worker。

**spec-19 的策略**：

- `mango task submit`（不带 `--wait`）：写入 DB 后**也启动一个 task_engine worker**，等到至少这个新任务从 pending 转到 running 才退出；之后 worker 不存活。如果用户希望 CLI 把任务跑完，必须用 `--wait`，CLI 进程内 worker 一直活到任务终态。
- `mango task submit --wait`：进程级 worker 跑到任务终态，结果文件下载完落地后退出。
- 任务进入 running 后，如果用户在 worker 还活着的时候 ctrl-c：spec-15 的 startup orphan reset 会在下次 `mango task submit/list/...` 时把它重置为 pending。结果：
  - GUI 之后启动 → worker 重新接管
  - 或下次 CLI `--wait` 提交时 → worker 再次处理它

**警告语**（CLI 在 `task submit` 完成时如果检测到 GUI 不在跑，给提示）：

```text
注意：未检测到 GUI 进程，且未指定 --wait。
任务已写入 DB（pending 状态）。下次启动 GUI 或运行 `mango task submit --wait <...>` 时会被处理。
```

> 「检测 GUI 是否在跑」简化成「判断 SQLite 上 `task_engine_lock` advisory key 是否被持有」。这是个 spec-15 需要补的轻量约定：task_engine 启动时插入一行 `SELECT 1 FROM task_engine_lock WHERE pid = ?`，CLI 检查表中是否存在过期 < 10s 的心跳。如不想引入额外表，**改为 best-effort：CLI 不检测，简单提示「如果 GUI 没在跑，请加 --wait」**。这是 spec-19 倾向方案——简单 > 精确。

### 3.5 Provider 注册

CLI 起 task_engine 时也需要注册 Provider —— 与 src-tauri/src/lib.rs 同样的代码。复用 spec-17 的 `bailian::create_provider(...)`，在 task.rs 的 worker 启动段执行：

```rust
let asset_uploader = oss_uploader::OssUploader::new(/* placeholder, 实际从 account 解析 */);
let provider = bailian::BailianProvider::new(Arc::new(asset_uploader));
let registry = ProviderRegistry::builder()
    .add("bailian", Arc::new(provider))
    .build();
let materializer = BailianResultMaterializer::new(...);
let (engine, _events_rx) = TaskEngineHandle::spawn(conn.clone(), registry, materializer, 4).await?;
```

> CLI 不消费 events，所以丢掉 `events_rx`（drop 即可，channel 关闭，发送端会观察到但不致命；spec-15 已要求 send 失败时仅 warn）。
>
> 注：`asset_uploader` 的真正初始化要把 OSS 账户私参从 `api_account.params_json` + `keyring("api_account:{id}:oss_secret")` 解析出来。这部分与 GUI 共享：抽取 `mango_core::provider::asset_uploader::oss::from_account_id(conn, account_id) -> OssUploader`。spec-16/17 的实施要把这个解析函数暴露在 core 公开 API。

### 3.6 `--json` 输出契约

所有支持 `--json` 的子命令必须保证：
- stdout **只有** JSON（人类可读说明全部走 stderr）
- 多行场景用 NDJSON（每行一对象），不用 `[...]` 数组（避免大 list 时 stdout 阻塞 buffer）
- task 对象 schema 与 `GenerationTask` ts-rs 导出的前端类型一一致；CLI 复用 `serde_json::to_string(&task)`，不自定义 schema

---

## 4. 错误处理

### 4.1 用户面向错误

- 账号不存在：`no account configured for provider 'bailian'. Run mango account add ...`
- 模型不识别：`unknown model id 'foo'. Supported: wan2.7-image-pro, happyhorse-1.0-r2v`
- happyhorse 缺主体：`--subject is required for happyhorse-1.0-r2v`
- 项目 ID 不存在：core 错误透传，加上 `--project-id <id> 不存在`
- 主体没有参考图：`subject <id> has no reference images; happyhorse requires at least one`

### 4.2 网络/API 错误

直接透传 spec-17 的 `CoreError::Provider(...)` 字符串；CLI 不做美化，但要在错误前缀加上「provider 错误」让用户看清是远端原因还是本地 bug。

### 4.3 退出码

- `0` 成功（包括 `--wait` 任务最终为 success）
- `1` 通用错误
- `2` 用户输入错误（clap parse 失败）
- `10` 任务最终为 failed（仅 `--wait` 模式；不带 `--wait` 提交成功就退出 0）
- `11` 任务最终为 cancelled（同上）

`--wait` 用户能通过退出码做 CI/脚本判断。

---

## 5. 与 task_service 的契约

spec-15 已经定义 task_service 模块（`mango_core::task_engine::service` 或类似命名）。spec-19 要的对外 sync-friendly async API：

```rust
pub async fn submit_task(conn: &TokioConn, app_data_dir: &Path, input: SubmitTaskInput) -> Result<GenerationTask>;
pub async fn get_task(conn: &TokioConn, task_id: &str) -> Result<GenerationTask>;
pub async fn list_tasks(conn: &TokioConn, filter: ListTaskFilter) -> Result<Vec<GenerationTask>>;
pub async fn cancel_task(conn: &TokioConn, task_id: &str) -> Result<()>;
```

GUI 的 Tauri command 包一层 `Result<_, String>` 转换；CLI 直接消费 `Result<_, CoreError>` 然后用 anyhow context。两边走同样的核心调用链。

---

## 6. 验收标准

来自任务 9：

- [ ] CLI 提交的任务在 GUI 任务面板可见（同一 SQLite + project）
- [ ] `mango task status <id>` 输出正确，watch 模式 5s 刷新一次
- [ ] `mango task list` 默认显示最近 7 天，过滤参数有效
- [ ] `mango task submit --wait` 阻塞到 success / failed 并按退出码区分
- [ ] 提交时 account/model/subject 校验缺失时给出明确提示（不是栈追踪）
- [ ] `--json` 模式 stdout 只有 JSON，无人类可读字符
- [ ] CLI 单独跑（GUI 关闭）时也能完成全链路（worker 在 CLI 进程内）

---

## 7. 测试

### 7.1 单元测试（crates/cli/tests/）

- `tests/cli_task_submit.rs`：用 `assert_cmd` + 内存 SQLite + mock provider
  - 测 happy path：stub provider 立刻 success，CLI `--wait` 退 0 + stdout 含 task_id
  - 测 failed：stub provider 返回 Failed，CLI `--wait` 退 10
  - 测缺账号、缺主体、未知模型的错误消息
- `tests/cli_task_list.rs`：插入若干 GenerationTask 行，验证 `--status` / `--since` / `--limit` 过滤
- `tests/cli_task_status.rs`：`--watch` 模式手动触发 status 变更，验证打印行序

### 7.2 端到端（手动，不进 CI）

- 真实 DashScope key，`mango task submit --wait` 跑 wan2.7 / happyhorse 各一次，确认结果文件下载到 `<project>/assets/`、Asset 行入库（spec-17 的 ResultMaterializer）

---

## 8. 帮助文本

`mango task --help`：

```text
mango-task 0.x.0
AI generation tasks (submit / poll / list / cancel)

USAGE:
    mango task <SUBCOMMAND>

SUBCOMMANDS:
    submit    Submit a new generation task
    status    Show task details (use --watch to follow)
    list      List recent tasks
    cancel    Cancel a running task

For per-subcommand options, run `mango task <SUBCOMMAND> --help`.
```

每个子命令的 `--help` 必须示例化最常用的两条命令（图 + 视频）—— 让用户看 help 就能拷贝执行。

---

## 9. 不在本 spec 范围

- `mango task export <id>`（导出结果到自定义路径）：MS5 视频导出阶段做
- `mango task watch-all`（实时 tail 所有任务状态变更）：MS4 批量功能
- 配置默认 provider/model（避免每次输 `--provider bailian`）：MS4 之后引入 `~/.mango/config.toml` 时合并
- shell 自动补全（`mango completions zsh`）：clap 自带，spec 不强制

---

## 10. 依赖增量

`crates/cli/Cargo.toml` 需新增：

- `tokio` (workspace) — 启动 runtime；spec-15 已加在 workspace
- `serde_json` (workspace) — JSON 输出
- `chrono` (workspace) — `--since` 解析时间
- `humantime` ≥ 2.x — 解析 `1d/30m/12h` 这类 duration（标准 chrono 不支持）

具体版本在 dependencies.md 汇总并 web 验证最新 stable。
