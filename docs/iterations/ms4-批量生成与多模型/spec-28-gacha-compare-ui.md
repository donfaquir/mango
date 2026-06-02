# SPEC-28: 批量抽卡对比 + 采用结果

> 对应 MS4 任务 5。同一分镜下多模型/多次生成的成功结果并排对比，一键「采用」为分镜最终成片素材。

依赖：spec-26（按 shot 聚合任务）+ MS3 spec-23（绑定语义升级）+ MS2 spec-18（结果预览）

非目标：
- 画布节点内嵌对比视图 — V2；本 spec 先做独立面板/Dialog
- 视频逐帧对比 — 仅缩略图 + 弹窗播放

---

## 1. Migration `010_shot_result_asset_id.sql`

```sql
ALTER TABLE shot ADD COLUMN result_asset_id TEXT REFERENCES asset(id) ON DELETE SET NULL;
CREATE INDEX idx_shot_result_asset ON shot(result_asset_id);
```

**语义分裂（对齐 spec-23）**：

| 操作 | 写入列 | 用途 |
|---|---|---|
| 拖入参考图 | `asset.shot_id` | 生成输入 |
| 采用生成结果 | `shot.result_asset_id` + 可选 `asset.shot_id` 保持 | 分镜「定稿」输出 |

`Shot` TS/Rust model 增加 `result_asset_id: Option<String>`。

---

## 2. Core：`adopt_task_result`

```rust
pub struct AdoptTaskResultInput {
    pub shot_id: String,
    pub task_id: String,
}

/// 1. 校验 task.status == success && task.shot_id == shot_id
/// 2. 校验 task.result_asset_id Some
/// 3. UPDATE shot SET result_asset_id = ?, status = 'done', updated_at = now
/// 4. 可选：UPDATE asset SET shot_id = shot_id（方便素材库筛选）
```

IPC：`adopt_task_result(input) -> Shot`。

---

## 3. 查询：`list_tasks_for_shot_compare`

```rust
pub struct ShotCompareTaskRow {
    // GenerationTask 字段 +
    pub model_name: String,
    pub result_thumbnail_path: Option<String>,
    pub result_asset_path: Option<String>,
}

/// SELECT ... FROM generation_task
/// JOIN model ON ...
/// LEFT JOIN asset ON result_asset_id
/// WHERE shot_id = ? AND status = 'success'
/// ORDER BY created_at DESC
/// LIMIT 50
```

可选 query param `batch_id` 仅看某一批抽卡结果。

---

## 4. UI：`GachaComparePanel`

**入口**：

- 分镜详情 / 画布 Storyboard 节点右键「对比生成结果」
- 批量完成后 toast action「去对比」

**布局**：

- CSS grid，`minmax(200px, 1fr)`，每张卡片：
  - 缩略图（`useResolvedAssetUrl`）
  - 模型名 + 相对时间
  - 「采用」主按钮；已采用项高亮边框
- 点击缩略图 → 复用 `ResultPreviewDialog`

**采用流程**：

1. `adoptTaskResult` mutation
2. toast「已采用为分镜结果」
3. invalidate `shots` + `tasks` queries

当前采用结果：`shot.result_asset_id` 对应卡片显示「当前采用」。

---

## 5. 与 spec-23 文案

- 拖放绑定仍用「确认绑定」→ `asset.shot_id`
- 本 spec 按钮文案：「采用为结果」
- `BindAssetDialog` 不再承担「采用」语义

---

## 6. 测试

| 层级 | 用例 |
|---|---|
| Rust | adopt 成功 → shot.status done + result_asset_id 设置 |
| Rust | adopt 非 success 任务 → 错误 |
| 前端 | 两模型两卡片；采用后另一张可切换采用 |

---

## 7. 验收清单

- [ ] migration 010 应用成功
- [ ] 同一分镜多模型 success 任务并排展示
- [ ] 可区分模型名与时间
- [ ] 一键采用后 `shot.status === 'done'` 且 `result_asset_id` 正确
- [ ] 可放大/播放预览（图/视频）
