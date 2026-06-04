# SPEC-31: CLI 检查点命令

> 对应 MS4 任务 10。`mango checkpoint list / create / restore`，与 GUI 共用 core 查询层。

依赖：spec-29

非目标：
- 自动 GC 触发 — 由应用 startup 负责，CLI 不单独暴露

---

## 1. CLI 结构

`crates/cli/src/commands/checkpoint.rs`（单文件，与 `project.rs` / `character.rs` 同模式）：

```rust
#[derive(clap::Subcommand)]
pub enum CheckpointAction {
    List(ListArgs),
    Create(CreateArgs),
    Restore(RestoreArgs),
    Delete(DeleteArgs),
}
```

`main.rs`：
- `Commands` 枚举增加 `Checkpoint(CheckpointArgs)`
- `let conn` 改为 `let mut conn`（`restore` 需要 `&mut Connection` 用于事务）

---

## 2. 子命令

### `mango checkpoint list --episode <UUID>`

`comfy_table` 表格（与 `project list` 同风格），列：id（短）, version, type, label, summary, created_at。

### `mango checkpoint create --episode <UUID> [--label "分镜定稿"]`

调 core `insert_full`；输出缩进键值对（与 `project create` 同风格）：

```
  ID:       <short_id>
  Version:  <version_number>
  Label:    <label 或 —>
  Summary:  <change_summary>
```

### `mango checkpoint restore <checkpoint_id> [--yes]`

- 无 `--yes`：stdin 提示确认（与 `project delete` 同模式）
- 调 `checkpoint_ops::restore(&mut conn, ...)`（含恢复前 auto 快照）
- 输出 `Restored episode <short_id> to version <n>`

### `mango checkpoint delete <checkpoint_id> [--yes]`

- 无 `--yes`：stdin 提示确认
- 调 core `delete`
- 输出 `Deleted checkpoint <short_id>`

---

## 3. 与 workspace / DB

CLI 与 GUI 共用 `com.mango.app/mango.db`（与 spec-19 一致）。

未 mount workspace 时 restore 仍可做 DB 层恢复；画布文件路径若依赖 project_root，需已配置 project。

---

## 4. 测试

| 用例 | 说明 |
|---|---|
| create → list | create 后 list 含新行 |
| create 无效 episode | 报错 episode not found |
| restore --yes | 恢复后 shots 行数与快照一致 |
| restore 无效 id | 报错 checkpoint not found |
| restore 无 --yes | stdin 非 yes 时拒绝 |
| delete --yes | 删除后 list 不含该行 |

---

## 5. 验收清单

- [ ] `checkpoint list` 表格显示完整字段
- [ ] `checkpoint create` 的行在 GUI 版本历史面板可见
- [ ] `checkpoint restore` 无 `--yes` 时拒绝；有 `--yes` 成功恢复
- [ ] `checkpoint delete` 删除后 GUI 列表同步消失
- [ ] 与 spec-29 GC / auto 逻辑不冲突
