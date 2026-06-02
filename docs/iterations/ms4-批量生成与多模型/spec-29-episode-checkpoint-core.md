# SPEC-29: EpisodeCheckpoint 核心（创建 / 恢复 / 自动保存 / 清理）

> 对应 MS4 任务 6、8、9。扩展 MS3 spec-24 的 `insert_minimal`，实现完整快照、恢复、change_summary、自动触发与 GC。

依赖：MS3 spec-24（占位 command + 表 schema）+ spec-20（episode/shot）+ spec-21（canvas_layout）

非目标：
- 版本历史面板 UI — spec-30
- CLI — spec-31
- `VACUUM INTO` 全库备份 — MS5 spec 范围（spec-24 非目标）

---

## 1. 版本号策略（决议）

`episode_checkpoint.version_number` **仍为全局自增**（manual + auto 共用序列），避免 migration。

UI 展示规则（spec-30 实现）：

- `trigger_type=manual`：显示「版本 {version_number}」
- `trigger_type=auto`：显示「自动备份 · {相对时间}」，**不强调版本号**

开发任务原「自动不计入手动序列」以 UI 过滤替代，不新增 `manual_version_number` 列。

---

## 2. 完整创建

扩展 `create_episode_checkpoint`（同名 command，替换占位实现）：

**快照内容**：

| 列 | 来源 |
|---|---|
| `script_text` | episode 表剧本字段（若 MS3 尚未有列，从 canvas ScriptNode 聚合或 episode.description — 实施时以已有 schema 为准；无则 `''`） |
| `shots_json` | `list_shots(episode_id)` 序列化 JSON |
| `canvas_*_json` | `canvas_layout` 当前行 |
| `change_summary` | 见 §3 |
| `trigger_type` | `manual` / `auto` |
| `label` | 用户输入；auto 可为 NULL |

`insert_full` 替换 `insert_minimal`，spec-24 测试改为断言非空 `shots_json`。

---

## 3. change_summary 生成

对比「上一手动检查点」与当前快照（无上一手动则对比空）：

```
分镜 8→10；剧本 +120 字；画布节点 12→14
```

算法（Rust）：

- `shots_json` 数组长度差
- `script_text` char count 差（Unicode scalar）
- `canvas_nodes_json` parse 后 `nodes.len()` 差

失败降级为 `有变更` 固定文案。

---

## 4. 恢复

`restore_episode_checkpoint(checkpoint_id) -> Episode`：

1. **恢复前自动检查点**：对目标 `episode_id` 调 `create_auto_checkpoint(episode_id, label=None)`（trigger=auto）
2. 读 checkpoint 行
3. 事务内：
   - 用 `shots_json` 全量替换该 episode 下 shots（策略：**删除未出现在 JSON 的 shot** 或软保留 — 决议用「全量覆盖」：DELETE shots WHERE episode_id AND id NOT IN json ids，再 UPSERT json 中各行）
   - 写回 `canvas_layout`
   - 写回 `script_text` 到 episode
4. commit
5. emit Tauri event `episode-data-restored { episode_id }` 供前端 invalidate canvas + shot queries

**风险**：全量覆盖破坏性大 — 恢复 Dialog 必须二次确认（spec-30）。

---

## 5. 删除

`delete_episode_checkpoint(id)` — 仅 DELETE 行，不删 asset 文件。

手动检查点可删；auto 也可删（用户清理）。

---

## 6. 自动保存

`crates/core/src/checkpoint/auto.rs`：

| 触发 | 函数 |
|---|---|
| 批量提交前 | `ensure_auto_before_batch(episode_id)` |
| restore 前 | 内置于 restore 流程 |
| 定时 30min | 前端 `useCheckpointAutoSave(episodeId)`：监听 canvasStore + shot query 变更 debounce，调 `create_auto_checkpoint_if_dirty` |

**dirty 检测**：episode 级 `last_checkpoint_hash` 存 app 内存或 preference 键 `checkpoint.last_hash.{episode_id}`；内容 hash = SHA256(script + shots + canvas json)。

无变更 → 不 INSERT。

---

## 7. GC 策略

`gc_auto_checkpoints(episode_id)` — 在 startup + 每次 `create_*_checkpoint` 后调用：

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
| `create_episode_checkpoint` | 扩展为 full（兼容 MS3 入参） |
| `create_auto_checkpoint` | 内部/前端定时用；`trigger_type=auto` |
| `list_episode_checkpoints` | `(episode_id) -> Vec<EpisodeCheckpointListItem>` |
| `restore_episode_checkpoint` | §4 |
| `delete_episode_checkpoint` | §5 |

`EpisodeCheckpointListItem` 含 `change_summary`, `trigger_type`, `version_number`, `label`, `created_at`。

---

## 9. 测试

| 用例 | 说明 |
|---|---|
| create full | shots_json 与 DB 一致 |
| restore | 恢复后 SELECT 与快照一致 |
| restore 前 auto 行 | 多一条 trigger=auto |
| GC | 8 天前 auto 只留每周最后一条；manual 仍在 |

---

## 10. 验收清单

- [ ] 手动创建保存 script + shots + canvas
- [ ] change_summary 非空且可读
- [ ] 恢复后数据与快照一致
- [ ] 恢复前自动创建当前状态检查点
- [ ] 30 分钟内有变更产生 auto 行；无变更不产生
- [ ] 批量/恢复前 auto 触发
- [ ] GC 不删 manual；auto 按规则压缩
