# SPEC-30: 版本历史面板 UI

> 对应 MS4 任务 7。保存版本标签输入、检查点列表、恢复确认、删除保护、手动/自动视觉区分。

依赖：spec-29（create / list / restore / delete）

非目标：
- 画布内嵌时间轴 — V2
- diff 可视化（逐字段对比）— V2
- 事后编辑标签 — 保存时已可输入，暂不需要 `update_checkpoint_label` 命令

---

## 0. 前置：挂载 spec-29 遗留 hooks

spec-29 实现了两个 hook 但未挂载到组件树，本 spec 开始前必须先接入：

| Hook | 挂载位置 | 作用 |
|---|---|---|
| `useCheckpointAutoSave(episodeId)` | `CanvasInner.tsx` | 30 分钟定时自动保存 |
| `useEpisodeDataRestoredListener()` | `CanvasInner.tsx` | 恢复后 invalidate shots/canvas/episode queries |

挂载后验证：
- 进入画布页 → 控制台无报错
- 恢复检查点后（可通过 DevTools 调 IPC），前端数据自动刷新

---

## 1. 组件结构

```text
src/components/checkpoint/
├── SaveVersionDialog.tsx         # 保存版本时填写标签
├── VersionHistoryDialog.tsx      # 检查点列表弹窗
├── CheckpointListItem.tsx        # 列表中的单条记录
└── RestoreConfirmDialog.tsx      # 恢复二次确认
```

所有弹窗使用项目现有 `Dialog` 组件（`@/components/ui/dialog`），与 DeleteAssetConfirmDialog、EditShotDialog 等保持一致。不引入 Sheet 或 AlertDialog。

---

## 2. 保存版本（改造现有流程）

**现状**：`CanvasToolbar` 的「保存版本」按钮直接调 `createCheckpoint.mutateAsync({ episode_id, label: null })`，无法输入标签。

**改造**：点击按钮 → 打开 `SaveVersionDialog` → 用户填写标签（可留空）→ 确认后创建 checkpoint。

### SaveVersionDialog

- Props：`open`, `onOpenChange`, `episodeId`, `onBeforeSave?: () => Promise<void> | void`
- 内容：单个 Input，placeholder "如：分镜定稿"，可留空
- 确认按钮："保存版本"（非 destructive）
- 流程：
  1. `await onBeforeSave?.()` — flush auto-save debounce
  2. `createCheckpoint.mutateAsync({ episode_id, label: inputValue || null })`
  3. `toast.success("已保存版本")`
  4. 关闭 dialog

### CanvasToolbar 改动

- 新增 `useState<boolean>` 控制 SaveVersionDialog 的 open 状态
- BookmarkPlus 按钮 onClick 改为 `setSaveDialogOpen(true)`
- 移除原 `onSaveVersion` handler，逻辑移入 SaveVersionDialog

---

## 3. 版本历史列表

### 入口

- `CanvasToolbar` 增加「版本历史」按钮（`History` 图标），与「保存版本」同组
- 点击打开 `VersionHistoryDialog`

### VersionHistoryDialog

- Props：`open`, `onOpenChange`, `episodeId`
- 数据：`useCheckpointList(episodeId)` → `list_episode_checkpoints`
- 排序：`created_at` 倒序（后端已保证）
- 空状态：居中提示"暂无版本记录"

### CheckpointListItem

每条记录显示：

| 字段 | manual | auto |
|---|---|---|
| 图标 | `Bookmark` | `Clock` |
| 标题 | `版本 {version_number}` + label（如有） | `自动备份` |
| 副标题 | `change_summary` | `change_summary` 或 `定时保存` |
| 时间 | 相对时间（如"2 小时前"） | 相对时间 |

操作按钮（右侧 DropdownMenu 或 inline buttons）：
- **恢复** → 打开 `RestoreConfirmDialog`
- **删除** → 见 §5 删除策略

---

## 4. 恢复确认

### RestoreConfirmDialog

- Props：`open`, `onOpenChange`, `checkpoint: EpisodeCheckpointListItem`, `episodeId`
- Confirm 按钮：`variant="destructive"`，label "确认恢复"

### 确认文案

```
将用「版本 {version_number} · {label}」覆盖当前集的所有分镜、画布与剧本。

系统会先自动保存当前状态。

⚠️ 恢复后，已有的生成结果将不再关联到分镜（抽卡对比中不可见）；
角色、场景、道具的分镜绑定需要重新设置。
```

- 无 label 时标题显示为「版本 {version_number}」
- auto checkpoint 标题显示为「自动备份 · {相对时间}」

### 恢复流程

1. 用户点确认
2. `restoreCheckpoint.mutateAsync(checkpoint.id)`
3. 后端：自动创建 auto checkpoint → 恢复数据 → emit `episode-data-restored` event
4. 前端：`useEpisodeDataRestoredListener` 自动 invalidate queries
5. `toast.success("已恢复到版本 {version_number}")`
6. 关闭 RestoreConfirmDialog 和 VersionHistoryDialog

---

## 5. 删除策略

区分 manual 和 auto，保护力度不同：

| 类型 | 确认方式 | 文案 |
|---|---|---|
| manual | Dialog 二次确认 | "删除「版本 {n} · {label}」后不可恢复，确认删除？" |
| auto | Dialog 二次确认（轻量） | "删除此自动备份记录？" |

删除成功后 `toast.success("已删除")`，列表自动刷新（`useDeleteCheckpoint` 已做 invalidate）。

---

## 6. 与 spec-24「保存版本」的关系

- 工具栏「保存版本」按钮行为改为：打开 SaveVersionDialog → 填标签 → 创建 full checkpoint（spec-29）
- 「版本历史」是只读列表 + 恢复/删除管理，不重复保存逻辑
- `CreateCheckpointInput` 结构不变：`{ episode_id, label, trigger_type }`

---

## 7. 不需要新增的后端命令

本 spec 仅消费 spec-29 已有的 4 个 IPC 命令：

| 命令 | 用途 |
|---|---|
| `create_episode_checkpoint` | SaveVersionDialog 创建 |
| `list_episode_checkpoints` | VersionHistoryDialog 列表 |
| `restore_episode_checkpoint` | RestoreConfirmDialog 恢复 |
| `delete_episode_checkpoint` | 列表项删除 |

不需要 `update_checkpoint_label`——标签在创建时输入，不支持事后修改。

---

## 8. 测试

| 类型 | 用例 |
|---|---|
| 前端 RTL | SaveVersionDialog：输入 label → 创建 checkpoint 时 label 正确传递 |
| 前端 RTL | SaveVersionDialog：留空 → label 为 null |
| 前端 RTL | VersionHistoryDialog：manual 显示 Bookmark 图标 + 版本号，auto 显示 Clock 图标 |
| 前端 RTL | RestoreConfirmDialog：取消不调 IPC |
| 前端 RTL | RestoreConfirmDialog：确认后调 `restoreEpisodeCheckpoint` |
| 前端 RTL | 删除 manual 需确认；确认后列表刷新 |

---

## 9. 验收清单

- [ ] `useCheckpointAutoSave` 和 `useEpisodeDataRestoredListener` 已挂载到 CanvasInner
- [ ] 「保存版本」按钮弹出 SaveVersionDialog，可输入标签
- [ ] 留空标签时创建的 checkpoint label 为 NULL
- [ ] 填写标签后创建的 checkpoint label 正确存储
- [ ] 「版本历史」按钮打开 VersionHistoryDialog，列表正确展示
- [ ] manual / auto 视觉可区分（图标 + 标题格式不同）
- [ ] 恢复有二次确认，文案包含数据丢失风险提示
- [ ] 恢复后画布、分镜、剧本数据自动刷新（listener 生效）
- [ ] 删除 manual checkpoint 需确认
- [ ] 删除后列表自动刷新
- [ ] CLI 创建的检查点（spec-31）在列表可见
