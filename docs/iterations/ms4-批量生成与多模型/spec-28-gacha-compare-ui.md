# SPEC-28: 抽卡对比 + 采用结果

> 对应 MS4 任务 5。同一分镜下多模型/多次生成的成功结果并排对比，一键「采用」为分镜最终定稿素材。

依赖：spec-26（batch_id + shot_id 聚合任务）+ MS2 spec-18（`ResultPreviewDialog` 复用）

非目标：
- 画布节点内嵌对比视图 — V2；本 spec 先做独立 Dialog
- 视频逐帧对比 — 仅缩略图 + 弹窗播放
- `shot.status` 自动流转 — adopt 只写 `adopted_asset_id`，不隐式改 status（见 §2）

---

## 1. Migration `010_shot_adopted_asset_id.sql`

```sql
ALTER TABLE shot ADD COLUMN adopted_asset_id TEXT REFERENCES asset(id) ON DELETE SET NULL;
CREATE INDEX idx_shot_adopted_asset ON shot(adopted_asset_id);
```

**列名选择**：用 `adopted_asset_id` 而非 `result_asset_id`，因为 `generation_task` 已有 `result_asset_id` 列（含义为"生成产物"）。两列含义不同：

| 列 | 所在表 | 语义 |
|---|---|---|
| `generation_task.result_asset_id` | 任务 | 生成产物（多个任务各有各的） |
| `shot.adopted_asset_id` | 分镜 | 用户从多个产物中选中的定稿 |

**与 `asset.shot_id` 的关系**：

| 操作 | 写入列 | 用途 |
|---|---|---|
| 拖入参考图 / 生成结果入库 | `asset.shot_id` | 生成输入、素材库关联 |
| 用户采用某个生成结果 | `shot.adopted_asset_id` | 分镜最终输出 |

`Shot` TS/Rust model 增加 `adopted_asset_id: Option<String>`。

---

## 2. Core：`adopt_task_result` / `unadopt_shot`

### adopt

```rust
pub struct AdoptTaskResultInput {
    pub shot_id: String,
    pub task_id: String,
}

/// 1. load task; 校验 task.status == Success
/// 2. 校验 task.shot_id == Some(shot_id)
/// 3. 校验 task.result_asset_id.is_some()
/// 4. UPDATE shot SET adopted_asset_id = task.result_asset_id, updated_at = now
///    WHERE id = shot_id
/// 5. 返回更新后的 Shot
```

**不自动改 `shot.status`**。原因：status 有独立的语义（draft/ready/generating/done），由用户或其他流程控制。adopt 只负责记录"选了哪个"，不对工作流状态做隐式副作用。用户可以 adopt 后在 EditShotDialog 手动标 done，也可以 adopt 后继续生成更多对比。

**切换采用**：对同一 shot 再次调用 adopt（传不同 task_id）直接覆盖 `adopted_asset_id`。无需额外 API。

### unadopt

```rust
/// UPDATE shot SET adopted_asset_id = NULL, updated_at = now WHERE id = ?
```

清空采用。前端「取消采用」按钮调用。

IPC：
- `adopt_task_result(input: AdoptTaskResultInput) -> Shot`
- `unadopt_shot(shot_id: String) -> Shot`

---

## 3. 查询：扩展现有 `ListFilter`

**不新增独立查询函数**。在 `TaskEngineHandle::ListFilter` 上增加 `shot_id` 字段，复用现有 `list()` 查询路径：

```rust
pub struct ListFilter {
    pub project_id: Option<String>,
    pub shot_id: Option<String>,      // 新增
    pub status: Option<GenerationTaskStatus>,
    pub limit: Option<u32>,
}
```

查询层 `generation_task::list()` 增加 `shot_id` 分支（`WHERE shot_id = ?`），与 `project_id` / `status` 任意组合。

前端对比面板调用：`list({ shot_id, status: "success" })` → 返回 `GenerationTask[]`，每条 task 的 `result_asset_id` 通过现有 `useAsset` + `useResolvedAssetUrl` 解析缩略图。**不做 JOIN，不引入新的返回类型**。

IPC：复用现有 `list_tasks` command，入参加 `shot_id` 可选字段。

---

## 4. UI：`GachaCompareDialog`

### 入口

1. **ShotCard**：当 shot 有 ≥1 个 success 任务时，卡片上显示小缩略图预览 + 点击弹出对比 Dialog
2. **批量完成后 toast action**：「去对比」→ 打开第一个有 success 结果的 shot 的对比 Dialog

### 布局

Dialog 内部 CSS grid，`minmax(180px, 1fr)`，每张卡片：
- 缩略图（`useResolvedAssetUrl`，点击弹出 `ResultPreviewDialog`）
- 模型名（从 `task.model_id` 映射）+ 相对时间
- 「采用」按钮（已采用项显示「当前采用」+ 高亮边框 + 「取消采用」按钮）

### 采用流程

1. 点击「采用」→ `adoptTaskResult` mutation
2. toast「已采用为分镜结果」
3. invalidate `shots` + `tasks` queries → ShotCard 自动刷新缩略图

### 取消采用

1. 已采用卡片上点击「取消采用」→ `unadoptShot` mutation
2. toast「已取消采用」
3. invalidate `shots` queries

---

## 5. ShotCard 增强

当 `shot.adopted_asset_id` 非空时，ShotCard 左侧显示一个小缩略图预览（采用的结果）：
- 复用 `useAsset(shot.adopted_asset_id)` + `useResolvedAssetUrl` 取图
- 点击缩略图 → 打开 `GachaCompareDialog`

当 shot 无 `adopted_asset_id` 但有 success 任务时，显示一个虚线边框占位 + 「查看生成结果」文字，点击打开 Dialog。

---

## 6. 测试

| 层级 | 用例 |
|---|---|
| Rust | adopt 成功 → `shot.adopted_asset_id` 正确设置 |
| Rust | adopt 非 success 任务 → 返回错误 |
| Rust | adopt task.shot_id 与入参 shot_id 不匹配 → 返回错误 |
| Rust | adopt task.result_asset_id 为 None → 返回错误 |
| Rust | 切换采用（连续两次 adopt 不同 task）→ 最后一次生效 |
| Rust | unadopt → `adopted_asset_id` 变 NULL |
| Rust | list tasks with shot_id filter → 只返回对应 shot 的任务 |
| 前端 | 两模型两卡片；采用后高亮；切换采用后高亮跟随 |
| 前端 | 取消采用后 ShotCard 缩略图消失 |

---

## 7. 验收清单

- [ ] migration 010 应用成功
- [ ] `list_tasks(shot_id=X, status=success)` 返回正确结果
- [ ] 同一分镜多模型 success 任务在 Dialog 中并排展示
- [ ] 可区分模型名与时间
- [ ] 点击采用后 `shot.adopted_asset_id` 正确
- [ ] 切换采用（选另一个结果）正常覆盖
- [ ] 取消采用后 `adopted_asset_id` 为 NULL
- [ ] 点击缩略图可放大/播放预览（复用 `ResultPreviewDialog`）
- [ ] ShotCard 上显示已采用结果的小缩略图
