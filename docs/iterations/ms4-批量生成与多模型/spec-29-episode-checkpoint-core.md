# SPEC-29: EpisodeCheckpoint 核心（创建 / 恢复 / 自动保存 / 清理）

> 对应 MS4 任务 6、8、9。扩展 MS3 spec-24 的 `insert_minimal`，实现完整快照、恢复、change_summary、自动触发与 GC。

依赖：MS3 spec-24（占位 command + 表 schema）+ spec-20（episode/shot）+ spec-21（canvas_layout）+ spec-28（`shot.adopted_asset_id`）

非目标：
- 版本历史面板 UI — spec-30
- CLI — spec-31
- `VACUUM INTO` 全库备份 — MS5 spec 范围（spec-24 非目标）

---

## 1. 版本号策略（决议）

`episode_checkpoint.version_number` 为 **per-episode 自增**（与 MS3 现有代码一致）。`manual` + `auto` 共用同一序列。

UI 展示规则（spec-30 实现）：

- `trigger_type=manual`：显示「版本 {version_number}」
- `trigger_type=auto`：显示「自动备份 · {相对时间}」，**不强调版本号**

---

## 2. 完整创建

扩展 `create_episode_checkpoint`（同名 command，替换占位实现）。`CreateCheckpointInput` 增加 `trigger_type: Option<String>`，默认 `manual`。不新增 `create_auto_checkpoint` command——前端 auto-save 传 `trigger_type: "auto"` 即可。

**快照内容**：

| 列 | 来源 |
|---|---|
| `script_text` | `episode.script_text` |
| `shots_json` | `list_shots(episode_id)` 序列化 JSON（含所有字段包括 `adopted_asset_id`） |
| `canvas_*_json` | `canvas_layout` 当前行；**无 canvas 时用空默认值**（`nodes="[]"`, `edges="[]"`, `viewport="{}"`），不报错 |
| `change_summary` | 见 §3 |
| `trigger_type` | `manual` / `auto` |
| `label` | 用户输入；auto 可为 NULL |

`insert_full` 替换 `insert_minimal`。

---

## 3. change_summary 生成

对比「上一条检查点」（不区分 manual/auto）与当前快照（无上一条则对比空）：

```
新增 2 个分镜（共 10）；修改 5 个分镜；剧本 +120 字；画布节点 12→14
```

算法（Rust）：

- **shots 变化**：parse 两份 `shots_json` 为 `Vec<Value>`，按 `id` 匹配：
  - 新增：当前有、上一份无的 id 数量
  - 删除：上一份有、当前无的 id 数量
  - 修改：id 相同但 JSON 内容不同的数量（直接比较序列化字符串）
- **script 变化**：`script_text` 的 Unicode scalar count 差值
- **canvas 变化**：`canvas_nodes_json` parse 后 `nodes.len()` 差值

组装为中文摘要。任一环节 parse 失败时该段降级为空，不影响其他段。全部为空则输出 `无变更`。

---

## 4. 恢复

`restore_episode_checkpoint(checkpoint_id) -> Episode`：

1. **恢复前自动检查点**：对目标 `episode_id` 调 `insert_full(episode_id, trigger_type=auto, label=None)`
2. 读 checkpoint 行
3. 事务内：
   - 用 `shots_json` 全量替换该 episode 下 shots（策略：DELETE 所有旧 shot → INSERT json 中各行）
   - 写回 `canvas_layout`（via `upsert`）
   - 写回 `script_text` 到 episode
4. commit
5. emit Tauri event `episode-data-restored { episode_id }` 供前端 invalidate canvas + shot queries

**⚠️ 已知副作用**：全量 DELETE shots 触发 `generation_task.shot_id ON DELETE SET NULL`——恢复时被删的 shot 关联的生成任务会丢失 `shot_id` 引用，这些任务不再出现在对比 Dialog 中。恢复 Dialog 必须二次确认并告知此风险（spec-30）。

**⚠️ shot 子表数据**：`shot_character` / `shot_scene` / `shot_prop` 联接表在 DELETE shot 时级联删除（`ON DELETE CASCADE`）。`shots_json` 不含联接关系，恢复后需要用户重新关联。如果这不可接受，需要在 `shots_json` 中额外序列化 `ShotLinks`——实施时决策。

---

## 5. 删除

`delete_episode_checkpoint(id)` — 仅 DELETE 行。

手动检查点可删；auto 也可删（用户清理）。

---

## 6. 自动保存

| 触发 | 方式 |
|---|---|
| 批量提交前 | `BatchSubmitDialog` 在 submit 前调 `create_episode_checkpoint(trigger_type=auto)` |
| restore 前 | 内置于 restore 流程（§4 step 1） |
| 定时 30min | 前端 `useCheckpointAutoSave(episodeId)` |

**dirty 检测**：存内存（`useRef<string>`）。`last_hash = SHA256(script + shots_json + canvas_json)`。重启后 hash 为空，首次有变更时会创建一条 auto checkpoint——这是可接受的，GC 会清理。

无变更 → 不 INSERT。

---

## 7. GC 策略

`gc_auto_checkpoints(episode_id)` — **每次创建 checkpoint 后**对同一 `episode_id` 执行（不在 startup 触发，避免扫描所有 episode 的复杂度）：

| 年龄 | 规则 |
|---|---|
| < 24h | 保留全部 auto |
| 1–7 天 | 每天 UTC 保留最后一条 auto |
| > 7 天 | 每周保留最后一条 auto |
| manual | **永不删除** |

实现：SQL 窗口函数或 Rust 分组后 DELETE ids。

---

## 8. IPC 一览

| command | 说明 |
|---|---|
| `create_episode_checkpoint` | 扩展为 full；`trigger_type` 可选（默认 manual） |
| `list_episode_checkpoints` | `(episode_id) -> Vec<EpisodeCheckpointListItem>` |
| `restore_episode_checkpoint` | §4 |
| `delete_episode_checkpoint` | §5 |

`EpisodeCheckpointListItem` 含 `id`, `version_number`, `trigger_type`, `label`, `change_summary`, `created_at`。不含 `shots_json` / `canvas_*_json`（列表不需要大 JSON）。

---

## 9. Model 变更

```rust
pub struct EpisodeCheckpoint {
    // 现有字段不变 +
    pub script_text: String,
    pub shots_json: String,
    pub change_summary: Option<String>,
}

pub struct EpisodeCheckpointListItem {
    pub id: String,
    pub episode_id: String,
    pub version_number: i64,
    pub trigger_type: String,
    pub label: Option<String>,
    pub change_summary: Option<String>,
    pub created_at: String,
}

pub struct CreateCheckpointInput {
    pub episode_id: String,
    pub label: Option<String>,
    pub trigger_type: Option<String>,  // 新增；默认 "manual"
}
```

---

## 10. 测试

| 用例 | 说明 |
|---|---|
| create full | `shots_json` 非空且与 DB shots 一致；`script_text` 与 episode 一致 |
| create without canvas | 无 canvas_layout 时仍成功，canvas 列为空默认值 |
| create auto | `trigger_type=auto` 正常写入 |
| change_summary | 新增/修改/删除分镜 + 剧本字数变化 → 摘要正确 |
| restore | 恢复后 shots、canvas、script 与快照一致 |
| restore 前 auto 行 | 恢复前多一条 `trigger_type=auto` |
| delete | DELETE 后查不到 |
| GC | 8 天前 auto 只留每周最后一条；manual 仍在 |
| list | 返回按 created_at DESC 排序，不含大 JSON 字段 |

---

## 11. 验收清单

- [ ] 手动创建保存 script + shots + canvas（含 adopted_asset_id）
- [ ] 无 canvas 时创建不报错
- [ ] change_summary 非空且可读（含分镜修改数量）
- [ ] 恢复后数据与快照一致
- [ ] 恢复前自动创建当前状态检查点
- [ ] 30 分钟内有变更产生 auto 行；无变更不产生
- [ ] 批量/恢复前 auto 触发
- [ ] GC 不删 manual；auto 按规则压缩
- [ ] list 返回轻量列表项
