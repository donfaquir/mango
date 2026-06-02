# SPEC-30: 版本历史面板 UI

> 对应 MS4 任务 7。检查点列表、恢复确认、标签编辑、手动/自动视觉区分。

依赖：spec-29（list / restore / delete / update label）

非目标：
- 画布内嵌时间轴 — V2
- diff 可视化（逐字段对比）— V2

---

## 1. 组件结构

```text
src/components/checkpoint/
├── VersionHistorySheet.tsx      # 右侧 Sheet，从画布工具栏「版本历史」进入
├── CheckpointListItem.tsx
├── RestoreCheckpointDialog.tsx  # AlertDialog 二次确认
└── EditCheckpointLabelDialog.tsx
```

**入口**：

- `CanvasToolbar` 增加「版本历史」按钮（与 spec-24「保存版本」并列）
- Episode 工作区菜单项

---

## 2. 列表 UX

`useEpisodeCheckpoints(episodeId)` → `list_episode_checkpoints`。

倒序 `created_at`；每项显示：

| 字段 | manual | auto |
|---|---|---|
| 标题 | `版本 {n}` + label | `自动备份` |
| 副标题 | change_summary | change_summary 或「定时保存」 |
| 图标 | Bookmark | Clock |

操作按钮：

- 恢复 → 打开 `RestoreCheckpointDialog`
- 删除 → Confirm（manual 提示不可恢复；auto 提示仅删记录）
- 编辑标签 → 仅 manual（auto 的 label 只读或隐藏编辑）

---

## 3. 恢复确认文案

```
将用「版本 3 · 分镜定稿」覆盖当前集的所有分镜与画布。
系统会先自动保存当前状态。此操作不可撤销。
```

确认后 `restoreEpisodeCheckpoint` → loading → success toast → 监听 `episode-data-restored` invalidate。

---

## 4. 与 spec-24「保存版本」

- 工具栏「保存版本」仍调 `create_episode_checkpoint`（spec-29 扩展后写 full 快照）
- 「版本历史」只读列表 + 管理，不重复保存逻辑

---

## 5. 测试

| 类型 | 用例 |
|---|---|
| 前端 RTL | 列表渲染 manual/auto 图标 |
| 前端 | 恢复 dialog 取消不调 IPC |
| E2E（可选） | 创建 → 列表可见 → 恢复后 canvas 节点数变化 |

---

## 6. 验收清单

- [ ] 检查点列表完整展示
- [ ] 恢复有二次确认
- [ ] 可修改 manual 检查点标签
- [ ] manual / auto 视觉可区分
- [ ] CLI 创建的检查点（spec-31）在列表可见
