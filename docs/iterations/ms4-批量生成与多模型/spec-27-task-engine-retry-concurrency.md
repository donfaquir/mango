# SPEC-27: 任务引擎 — 可配置并发 + 自动重试

> 对应 MS4 任务 3、4。将 MS2 已存在的全局 `Semaphore` 配置化，并在 runner 内对可重试错误做有限次指数退避重试。

依赖：MS2 spec-15 + `provider/error.rs`（`ProviderErrorKind`）

非目标：
- 替换调度架构（仍 per-task 协程 + semaphore，不引入独立 worker 进程）
- 修改孤儿 running 恢复语义（启动时 `reset_orphan_running` 保持 spec-15 行为）

---

## 1. Migration `011_app_preference.sql`

```sql
CREATE TABLE app_preference (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL
);
INSERT INTO app_preference (key, value_json) VALUES ('task.max_concurrency', '3');
```

> 默认从当前硬编码 `4` 调整为文档约定 `3`；升级用户首次 migration 写入 3，可在设置页改回 4。

Core：`db/queries/app_preference.rs` — `get_i64` / `set_i64`。

---

## 2. 可配置并发

**现状**（`src-tauri/src/init.rs`）：`TaskEngineHandle::spawn(..., 4)` 写死。

**目标**：

```rust
let max = app_preference::get_i64(conn, "task.max_concurrency").unwrap_or(3) as usize;
let (engine, rx) = TaskEngineHandle::spawn(..., max);
```

**动态调整（决议）**：

- 设置页修改后写入 DB + 调 `engine.set_max_concurrency(n)`：
  - 新 `Semaphore::new(n)` 替换 `Arc` 内 semaphore（或 `Semaphore::add_permits` 调整差额）
  - **已在跑的任务不中断**；仅影响新 acquire
- 若实现成本高，验收降级为「重启应用后生效」，须在 spec 验收清单标注

**前端**：`SettingsPage` 或 `GenerationSettingsCard` — 数字输入 1–8，保存调 `set_task_max_concurrency` IPC。

**队列顺序**：保持 FIFO — `recover_pending` 与 batch 提交按 `created_at ASC` 启动 runner。

---

## 3. 自动重试

### 3.1 可重试分类

在 `ProviderErrorKind` 上增加：

```rust
pub fn is_retryable(self) -> bool {
    matches!(self, Self::RateLimited | Self::Network | Self::Timeout)
}
```

`Unknown` 且 HTTP 5xx 可由 runner 在 `fail_with_detail` 路径额外判断 `http_status >= 500`。

### 3.2 Runner 行为

失败分支（`failed` 终态 **之前**）：

1. 若 `retry_count >= MAX_RETRIES`（3）→ 落库 failed，结束
2. 若 `!detail.kind.is_retryable()` → 立即 failed
3. 否则 `sleep(backoff(retry_count))` — 1s / 2s / 4s
4. `retry_count += 1`，`status = pending`，**保留 `external_task_id`** 仅当错误发生在 poll 阶段且 vendor 任务仍有效；submit 阶段失败则清空 `external_task_id` 后 re-submit

**与孤儿恢复区分**：

| 场景 | retry_count | 行为 |
|---|---|---|
| 启动 orphan reset | 不变 | 变 pending，由 `recover_pending()` 自动重新 spawn runner；resume 路径沿用 `external_task_id` 直接进 poll_loop（spec-15 ab07699 决议：常规恢复不算失败） |
| 自动重试 | +1 | 变 pending，sleep(backoff) 后 spawn runner；同样可走 resume 路径（poll 阶段失败时） |

P2：可在 `error_message` 追加 `[auto-retry n/3]` 标记便于诊断。当前实现重试时清空 `error_message`，最终失败时只写 `detail.to_string()`，看不出是否经过重试 —— 仅 `retry_count` 字段能区分。

### 3.3 状态查询

`list_tasks` / 任务卡片展示 `retry_count > 0` 时 badge「已重试 n 次」。

### 3.4 行为变更（⚠️ 与 MS2 不同）

**poll 阶段 transient 错误的语义变了**：

| 阶段 | MS2 行为 | spec-27 行为 |
|---|---|---|
| poll 调用本身报错（CoreError） | 记 warn event + `continue` 永久轮询 | 进 `fail_with_detail` → 走重试预算 |

也就是说 **3 次累计 retryable 失败后任务 final failed**。长时跑的视频任务（如 10 分钟生成）若期间出现 ≥3 次零星抖动会被误杀。

后续若实际跑量证明误杀频繁，可加「滑窗 + 临近 N 次失败才退」逻辑（P2）。

---

## 4. IPC

| command | 说明 |
|---|---|
| `get_task_max_concurrency` | 读 preference |
| `set_task_max_concurrency` | 写 preference + 调 engine |

---

## 5. 测试

| 用例 | 方式 | 状态 |
|---|---|---|
| stub provider 前 2 次 poll 返回 Network，第 3 次 Success | retry_count == 2 | ✅ `retries_network_errors_until_success` |
| Auth 错误 | retry_count 不变，直接 failed | ✅ `auth_errors_do_not_retry` |
| semaphore=1，同时 submit 3 任务 | 仅 1 行 running | ✅ `semaphore_caps_concurrent_runners_at_one` |
| Unknown + http_status=503 | 走重试路径 | ✅ `unknown_5xx_errors_are_retried` |
| 重试耗尽（3 次都失败） | 最终 failed，retry_count == 3 | ✅ `retry_exhaustion_caps_at_max_then_fails` |
| set_max_concurrency 实时切换 | 1→3 后新任务可同时跑 3 个 | ✅ `set_max_concurrency_allows_new_tasks_at_higher_cap` |

---

## 6. 验收清单

- [x] 同时 running 数 ≤ 配置值（默认 3）
- [x] 设置页可修改并发，新任务受新限制约束（**实时生效**，无需重启）
- [x] 网络/限流类错误自动重试，间隔 1s→2s→4s
- [x] 认证/配额类错误不重试
- [x] 超过 3 次后标记 failed，`retry_count` 正确
