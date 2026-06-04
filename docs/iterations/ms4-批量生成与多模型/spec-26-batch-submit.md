# SPEC-26: 批量任务提交（batch_id）

> 对应 MS4 任务 2。支持「多分镜 × 多模型（百炼内）」一次提交，用 `batch_id` 分组，提交前确认任务数量。

依赖：spec-25 + MS2 spec-15（TaskEngineHandle::submit）+ MS3 spec-20（shot 列表）

非目标：
- CSV 导入批量 — V2（spec-19 已明确 MS4 不做）
- 跨 Provider 批量 — 不在 MS4 范围

---

## 1. Migration `009_generation_task_batch_id.sql`

```sql
ALTER TABLE generation_task ADD COLUMN batch_id TEXT;
CREATE INDEX idx_generation_task_batch ON generation_task(batch_id);
```

- `batch_id` 可为 NULL（MS2 历史单条提交保持 NULL）
- 新批量提交：同一批共用一个 UUID v4

`GenerationTask` model + queries + specta 绑定同步加 `batch_id: Option<String>`。

---

## 2. Core API

`crates/core/src/task_engine/batch.rs`（或 `task_service::submit_batch`）：

```rust
pub struct SubmitBatchInput {
    pub project_id: String,
    pub account_id: String,
    pub provider_id: String,  // 固定 "bailian"
    pub shot_ids: Vec<String>,
    pub model_ids: Vec<String>,
    pub task_type: TaskKind,
    /// 若 Some，对所有 (shot, model) 使用该 JSON；否则按 shot 字段拼默认 prompt
    pub params_json_template: Option<String>,
}

pub struct SubmitBatchOutcome {
    pub batch_id: String,
    pub task_ids: Vec<String>,
    pub created_count: usize,
}
```

**展开规则**：笛卡尔积 `shot_ids × model_ids`，每个组合：

1. `CreateGenerationTaskInput { project_id, shot_id, provider_id, model_id, account_id, task_type, params_json, batch_id }`
2. 调现有 `submit`（复用校验 + runner 拉起）
3. 任一 insert 失败 → **整批事务回滚**（同一 `rusqlite` 事务内 INSERT，commit 后再逐个 `engine.submit(task_id)`）

**params 来源（决议）**：

| task_type | 默认 params 构造 |
|---|---|
| `image` | `shot.image_prompt` → wan27 字段；`size` 取 model default |
| `video` | `shot.video_prompt` + 从 `shot_character` 解析 reference assets（复用 spec-18 `resolveCharacterReferenceAssets` 逻辑，抽到 shared util） |

空 prompt 的分镜：跳过并在 outcome 中返回 `skipped: Vec<{ shot_id, reason }>`，不阻断其他组合。

---

## 3. Tauri command

```rust
#[tauri::command]
#[specta::specta]
pub async fn submit_tasks_batch(
    state: State<'_, AppState>,
    input: SubmitBatchInput,
) -> Result<SubmitBatchOutcome, IpcError> { ... }
```

批量提交前（若 spec-29 已合）调用 `checkpoint::ensure_auto_before_batch(episode_id)` — 软依赖，未合入则跳过。

---

## 4. 前端：批量生成入口

**入口 A（画布 / 分镜列表）**：多选 shot checkbox → 「批量生成」→ Dialog：

1. 选择 task_type（image | video）
2. `MultiModelSelect`（spec-25）
3. 选择账号（复用 ModelPicker 账号逻辑）
4. 确认页：`将创建 {n} 个任务（{shots} 分镜 × {models} 模型）`；P2 显示「预估消耗：待 usage 打通」

提交后 toast + 跳转任务面板并按 `batch_id` 过滤（任务列表加 batch 筛选 chip）。

**入口 B**：项目工作区任务面板内「批量模式」开关 — 与入口 A 共用 mutation。

`useSubmitTasksBatch` → `commands.submitTasksBatch`。

---

## 5. CLI（可选本 spec 合入）

```bash
mango task submit-batch \
  --project <id> \
  --account <id> \
  --type image \
  --shots shot1,shot2 \
  --models wan2.7-image-pro,happyhorse-1.0-r2v
```

与 GUI 共用 `SubmitBatchInput` 构造器。

---

## 6. 测试

| 层级 | 用例 |
|---|---|
| Rust integration | 2 shots × 2 models → 4 rows 同 batch_id；stub provider 全部 pending |
| Rust | 空 prompt shot 被 skip，其余仍创建 |
| 前端 | 确认 dialog 显示正确 N；提交后 listTasks 可按 batch 过滤 |

---

## 7. 验收清单

- [ ] migration 009 应用成功
- [ ] 可选中多个分镜 + 多个百炼模型，一次创建 N×M 任务
- [ ] 同批 `batch_id` 相同，单条提交仍为 NULL
- [ ] 提交前有确认提示，展示将创建的任务数
- [ ] 与 MS2 单条 `submit_task` 并存且不破坏旧数据
