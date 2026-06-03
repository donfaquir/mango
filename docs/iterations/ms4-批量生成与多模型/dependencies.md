# MS4 新增依赖一览

本文档汇总 MS4 七个 spec（25–31）中提议引入的依赖与 migration，便于一次性评审。**版本号在实施 PR 时按项目规则核对最新 stable 后再 pin**。

---

## 1. npm 依赖（`package.json`）

| 包 | 用途 | 引入 spec | 备注 |
|---|---|---|---|
| 无新增（预期） | 抽卡对比、版本面板复用 shadcn + TanStack Query | spec-28, spec-30 | 若对比视图需要虚拟滚动再评估 `@tanstack/react-virtual` |

### 1.1 shadcn/ui 组件（按需 `pnpm dlx shadcn@latest add ...`）

| 组件 | 用途 | 引入 spec |
|---|---|---|
| `scroll-area` | 版本历史侧栏长列表 | spec-30 |
| `alert-dialog` | 恢复检查点二次确认（若项目尚未 add） | spec-30 |

实施前用 `Glob src/components/ui` 核对是否已有，避免重复 add。

---

## 2. Rust 依赖（workspace）

| crate | 用途 | 引入 spec | 备注 |
|---|---|---|---|
| 无新增（预期） | 批量/检查点/重试复用 MS2 栈 | spec-25–31 | `uuid` / `serde_json` / `chrono` 已在 workspace |

---

## 3. Tauri 插件

无新增 Tauri 插件。

---

## 4. 数据库 migration

| migration | 内容 | 引入 spec |
|---|---|---|
| `009_generation_task_batch_id.sql` | `generation_task.batch_id TEXT` + 索引 | spec-26 |
| `010_shot_adopted_asset_id.sql` | `shot.adopted_asset_id TEXT REFERENCES asset(id)` | spec-28 |
| `011_app_preference.sql` | `app_preference(key TEXT PRIMARY KEY, value_json TEXT NOT NULL)`，首行 `task.max_concurrency = 3` | spec-27 |

`episode_checkpoint` 表 **不改 schema**（MS0 `001_initial.sql` + MS3 spec-24 占位已对齐）。

---

## 5. Tauri command 增量（一览）

| command | 引入 spec | 用途 |
|---|---|---|
| `submit_tasks_batch` | spec-26 | 批量创建 N×M 任务，返回 `batch_id` + task id 列表 |
| `adopt_task_result` | spec-28 | 写 `shot.adopted_asset_id`（不改 status） |
| `unadopt_shot` | spec-28 | 清空 `shot.adopted_asset_id` |
| `list_episode_checkpoints` | spec-29 | 版本列表 |
| `restore_episode_checkpoint` | spec-29 | 恢复（恢复前自动 auto-checkpoint） |
| `delete_episode_checkpoint` | spec-29 | 删除单条 |
| `update_episode_checkpoint_label` | spec-30 | 重命名标签 |

`create_episode_checkpoint` 由 spec-24 引入，spec-29 **扩展实现**（写入 `script_text` / `shots_json` / `change_summary`），不新增 command 名。

---

## 6. CLI 子命令增量

| 命令 | 引入 spec |
|---|---|
| `mango task submit-batch` | spec-26（可选，与 GUI 共用 core） |
| `mango checkpoint list / create / restore` | spec-31 |

---

## 7. 建议 PR 实施顺序

```
spec-25 (多模型元数据)
    └─▸ spec-27 (并发配置 + 自动重试)     ← 可与 spec-26 并行，但合入前 runner 行为要稳定
            └─▸ spec-26 (批量提交 + batch_id)
                    └─▸ spec-28 (抽卡对比 + adopt)
spec-29 (Checkpoint core + 自动保存 + GC)
    └─▸ spec-30 (版本面板 UI)
            └─▸ spec-31 (CLI checkpoint)
```

- **spec-26 依赖 spec-25**：批量 UI 需要多模型选择与参数模板。
- **spec-28 依赖 spec-26**：对比视图按 `shot_id` + 近期 batch 任务聚合。
- **spec-29 可与 spec-26 并行**（不同模块），但批量提交前应 hook「重大操作前 auto-checkpoint」（spec-29 提供 `ensure_auto_checkpoint_before`）。
- **spec-30/31 依赖 spec-29** 的 list/restore/delete 查询层。

---

## 8. 版本验证清单（实施 PR 前勾选）

- [ ] `pnpm outdated` — 无 MS4 强制升级的 npm 包时标 N/A
- [ ] `cargo outdated` — workspace 无新增 crate 时标 N/A
- [ ] migration 009–011 在空库 + 已有 008 的库上各跑一遍 `startup::initialize`
