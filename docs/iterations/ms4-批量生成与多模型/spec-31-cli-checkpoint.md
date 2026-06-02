# SPEC-31: CLI 检查点命令

> 对应 MS4 任务 10。`mango checkpoint list / create / restore`，与 GUI 共用 core 查询层。

依赖：spec-29

非目标：
- `mango checkpoint delete` — V2（GUI 已有则可顺手加，非本 spec 必须）
- 自动 GC 触发 — 由应用 startup 负责，CLI 不单独暴露

---

## 1. CLI 结构

`crates/cli/src/commands/checkpoint/mod.rs`：

```rust
#[derive(clap::Subcommand)]
pub enum CheckpointArgs {
    List(ListArgs),
    Create(CreateArgs),
    Restore(RestoreArgs),
}
```

`main.rs` `Commands` 枚举增加 `Checkpoint(CheckpointArgs)`。

---

## 2. 子命令

### `mango checkpoint list --episode <UUID> [--json]`

表格列：id, version_number, trigger_type, label, change_summary, created_at。

`--json` 输出与 GUI 列表同结构的 JSON 数组。

### `mango checkpoint create --episode <UUID> [--label "分镜定稿"]`

调 core `create_episode_checkpoint` full 实现；stdout 打印新 `id` + `version_number`。

### `mango checkpoint restore <checkpoint_id> [--yes]`

- 无 `--yes`：stdin 提示 `输入 yes 确认`
- 调 `restore_episode_checkpoint`（含恢复前 auto 快照）
- 成功打印 `restored episode_id=...`

---

## 3. 与 workspace / DB

CLI 与 GUI 共用 `com.mango.app/mango.db`（与 spec-19 一致）。

未 mount workspace 时 restore 仍可做 DB 层恢复；画布文件路径若依赖 project_root，需已配置 project。

---

## 4. 测试

| 用例 | 说明 |
|---|---|
| integration | create → list 含新行 |
| integration | restore --yes → shots 行数与快照一致 |

---

## 5. 验收清单

- [ ] `checkpoint list` 显示完整字段
- [ ] `checkpoint create` 的行在 GUI 版本面板可见
- [ ] `checkpoint restore` 无 `--yes` 时拒绝；有 `--yes` 成功恢复
- [ ] 与 spec-29 GC / auto 逻辑不冲突
