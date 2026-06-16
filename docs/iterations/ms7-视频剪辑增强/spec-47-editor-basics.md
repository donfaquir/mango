# SPEC-47: 编辑器基础操作补全

> 补齐多轨编辑器在 spec-44 落地后遗留的基础操作缺口。这些是用户日常剪辑的"table stakes"能力，优先级高于任务 4-8 的高级功能。

依赖：spec-44（多轨 UI 已落地）、spec-46（可视化已落地）

非目标：
- 代理渲染预览（proxy preview）— 当前源文件直播方案够用，代理渲染延后
- 导出参数 UI — spec-52
- 关键帧/转场/文字编辑面板 — spec-48/49/50
- 废弃组件清理 — 不阻塞功能，独立 PR 处理

---

## 1. 片段删除

### 1.1 键盘删除

**文件**：`src/components/editor/MultiTrackEditor.tsx`

在已有 `keydown` handler 中追加：

- `Delete` / `Backspace`：删除所有选中的 items（`selection` 非空时）
- 删除前检查 `locked` 状态，锁定轨道上的 item 跳过
- 批量删除：遍历 `selection`，依次调用 `removeItem(id)`

### 1.2 右键上下文菜单

**新建**：`src/components/editor/ItemContextMenu.tsx`（~60 行）

使用 shadcn `ContextMenu` 组件包裹 `TimelineItemBlock`：

| 菜单项 | 快捷键提示 | 动作 |
|--------|-----------|------|
| 删除 | Del | `removeItem(id)` |
| 分割 | S | `splitItem(id, playhead)` — 见 §2 |
| 复制 | Ctrl+D | `duplicateItem(id)` — 见 §3 |

锁定轨道时菜单项 disabled。

**文件修改**：`TimelineItemBlock.tsx` — 用 `<ItemContextMenu>` 包裹现有 div。

---

## 2. 分割片段

### 2.1 Store action

**文件**：`src/stores/multiTrackStore.ts`

新增 `splitItem(id: string, atMs: number): Promise<void>`：

1. 读取 item，校验 `atMs` 在 `[position_ms, position_ms + duration_ms]` 范围内
2. 计算分割点在 source 中的位置：`splitSourceMs = in_point_ms + (atMs - position_ms)`
3. 更新原 item：`out_point_ms = splitSourceMs`，`duration_ms = atMs - position_ms`
4. 创建新 item：`position_ms = atMs`，`in_point_ms = splitSourceMs`，保留原 `out_point_ms` 和 `asset_id`
5. 两步操作合并为一个 undo 步骤（pause/resume pattern 不适用；直接在一次 `set()` 中同时修改）

### 2.2 后端支持

**不需要新增后端命令**——用现有 `updateTimelineItem` + `createTimelineItem` 组合完成。

### 2.3 键盘快捷键

`S` 键（非 input/textarea 焦点时）：对所有选中 item 在 playhead 处分割。

---

## 3. 复制片段

### 3.1 Store action

新增 `duplicateItem(id: string): Promise<void>`：

1. 读取原 item 的所有属性
2. 创建新 item：`position_ms = 原 position_ms + 原 duration_ms`（紧跟其后），其余属性拷贝
3. 选中新 item

### 3.2 键盘快捷键

`Ctrl+D` / `Cmd+D`。

---

## 4. Undo/Redo 快捷键 + 按钮

### 4.1 键盘绑定

**文件**：`src/components/editor/MultiTrackEditor.tsx`

在 `keydown` handler 中追加：

- `Ctrl+Z` / `Cmd+Z`：`useMultiTrackStore.temporal.getState().undo()`
- `Ctrl+Shift+Z` / `Cmd+Shift+Z`：`useMultiTrackStore.temporal.getState().redo()`

### 4.2 工具栏按钮

**文件**：`src/components/editor/MultiTrackControls.tsx`

在传输控件左侧添加 Undo/Redo 图标按钮（Lucide `Undo2` / `Redo2`）。

读取 `useMultiTrackStore.temporal` 的 `pastStates.length` / `futureStates.length` 控制 disabled 状态。

---

## 5. 音频预览播放

### 5.1 Hook

**新建**：`src/hooks/usePlayheadAudioClips.ts`（~40 行）

从 `multiTrackStore` 读取所有非 muted 的 audio track，在每条轨道上找覆盖 playhead 的 clip item，返回 `{ assetId, audioUrl, seekMs }[]`。

逻辑与 `usePlayheadClip.ts` 对称——后者找视频，本 hook 找音频。

### 5.2 Audio 元素

**文件**：`src/components/editor/VideoPreview.tsx`

在 `<video>` 元素旁边渲染 N 个隐藏 `<audio>` 元素（N = 当前 playhead 覆盖的音频 clip 数，通常 1-3）。

- 每个 `<audio>` 的 src 绑定到对应 clip 的 asset URL
- play/pause 与 `isPlaying` 同步
- seek 逻辑同视频：暂停时精确 seek，播放时偏差 >300ms 时修正
- clip 变化时切换 src

---

## 6. 文本预览叠加

### 6.1 Hook

**新建**：`src/hooks/usePlayheadTextItems.ts`（~30 行）

找所有非 muted 的 text track 上覆盖 playhead 的 text items，返回 `{ content, textType, positionPct }[]`。

### 6.2 Overlay

**文件**：`src/components/editor/VideoPreview.tsx`

在视频区域上方叠加 CSS 绝对定位的文本层：

- subtitle → 底部居中，白字黑描边
- bubble → 居中，带白色圆角背景
- fancy / onomatopoeia → 居中，加粗大字

这是简化预览——spec-50 实现完整样式系统后替换。

---

## 7. 裁剪手柄（Trim Handles）

### 7.1 UI

**文件**：`src/components/editor/TimelineItemBlock.tsx`

在 item 块的左右两端各添加 4px 宽的拖拽 handle（仅选中态或 hover 时显示）：

- **左 handle**：拖拽改变 `in_point_ms` 和 `position_ms`（item 起始后移，源头裁剪）
- **右 handle**：拖拽改变 `out_point_ms` 和 `duration_ms`（item 末端裁剪）

拖拽约束：
- `in_point_ms >= 0`
- `out_point_ms <= source_duration`（需 probe 获取源文件时长，降级为不限制）
- `duration_ms >= 100`（最小 100ms）

### 7.2 数据更新

拖拽松手时调用 `updateItem(id, { in_point_ms, out_point_ms, position_ms, duration_ms })`。

---

## 8. 跨轨道拖拽

### 8.1 机制

**文件**：`src/components/editor/TimelineItemBlock.tsx` + `MultiTrackTimeline.tsx`

当前水平拖拽已实现。扩展为二维拖拽：

- 监听 `mousemove` 的 Y 轴偏移，计算目标轨道（基于轨道行高度 48px）
- 拖拽时视觉反馈：半透明 ghost 跟随鼠标
- 松手时调用 `moveItem(id, { track_id: targetTrackId, position_ms: newPos })`
- 类型约束：clip 只能在同类型轨道间移动（video→video，audio→audio）

---

## 9. 吸附对齐

### 9.1 逻辑

**新建**：`src/components/editor/snapUtils.ts`（~40 行）

`findSnapTarget(positionMs, allItems, playhead, threshold)` → `number | null`

吸附目标（优先级高→低）：
1. Playhead 位置
2. 其他 item 的 start（position_ms）
3. 其他 item 的 end（position_ms + duration_ms）
4. 整秒标记

吸附阈值：`pixelToMs(8, zoom)`（8 像素）。

### 9.2 应用

在 `TimelineItemBlock.tsx` 的 `mousemove` handler 中，计算新位置后调用 `findSnapTarget`，如果匹配则修正到吸附位置。

吸附时显示竖直辅助线（CSS absolute，z-index 高于 item）。

---

## 10. 轨道静音/锁定持久化

### 10.1 当前问题

`TrackHeader.tsx` 中 `toggleMuted` / `toggleLocked` 仅更新前端 store，不调用后端。

### 10.2 修复

**文件**：`src/stores/multiTrackStore.ts`

新增 `toggleMuted(trackId)` 和 `toggleLocked(trackId)` actions：

1. 翻转 store 中 track 的 `muted` / `locked` 值
2. 调用 `commands.reorderTimelineTracks` 或新增专用 IPC（后端 `timeline_track::update_muted` / `update_locked` 查询已存在，但无对应 Tauri 命令）

### 10.3 需新增 Tauri 命令

**文件**：`src-tauri/src/commands/timeline.rs`

```rust
pub async fn update_track_muted(id: String, muted: bool) -> Result<(), String>
pub async fn update_track_locked(id: String, locked: bool) -> Result<(), String>
```

后端查询函数 `timeline_track::update_muted` / `update_locked` 已存在，只需添加 Tauri 命令包装。

---

## 11. 目录增量

### 新建文件

| 文件 | 行数（估） | 职责 |
|------|-----------|------|
| `src/components/editor/ItemContextMenu.tsx` | ~60 | 右键上下文菜单 |
| `src/components/editor/snapUtils.ts` | ~40 | 吸附对齐计算 |
| `src/hooks/usePlayheadAudioClips.ts` | ~40 | 播放头音频 clip 解析 |
| `src/hooks/usePlayheadTextItems.ts` | ~30 | 播放头文本 item 解析 |

### 修改文件

| 文件 | 变更 |
|------|------|
| `src/stores/multiTrackStore.ts` | +splitItem, +duplicateItem, +toggleMuted, +toggleLocked |
| `src/components/editor/MultiTrackEditor.tsx` | +键盘快捷键（Delete/S/Ctrl+Z/Ctrl+D） |
| `src/components/editor/MultiTrackControls.tsx` | +Undo/Redo 按钮 |
| `src/components/editor/TimelineItemBlock.tsx` | +裁剪手柄 +跨轨道拖拽 +右键菜单 +吸附 |
| `src/components/editor/VideoPreview.tsx` | +`<audio>` 元素 +文本叠加层 |
| `src/components/editor/TrackHeader.tsx` | mute/lock 调用持久化 action |
| `src-tauri/src/commands/timeline.rs` | +update_track_muted, +update_track_locked |
| `src-tauri/src/lib.rs` | 注册 2 个新命令 |

---

## 12. 测试

| 用例 | 说明 |
|------|------|
| `delete_selected_items` | 选中 2 个 item → Delete → 删除成功，store 更新 |
| `delete_respects_lock` | 锁定轨道 item 不被删除 |
| `split_item_at_playhead` | 分割后两个 item 时间轴正确衔接 |
| `split_outside_range_noop` | playhead 不在 item 范围内时无操作 |
| `duplicate_item` | 复制后新 item 紧跟原 item |
| `undo_redo_keyboard` | Ctrl+Z 撤销最后操作，Ctrl+Shift+Z 重做 |
| `snap_to_playhead` | 拖拽接近 playhead 时吸附 |
| `snap_to_item_edge` | 拖拽接近其他 item 边界时吸附 |
| `trim_handle_left` | 左 handle 拖拽修改 in_point_ms + position_ms |
| `trim_handle_right` | 右 handle 拖拽修改 out_point_ms + duration_ms |
| `cross_track_drag` | 拖拽 item 到另一条同类型轨道 |
| `mute_persists` | 点击 mute → 刷新 → 仍为 muted |

---

## 13. 验收清单

- [ ] Delete 键删除选中片段，批量删除正常
- [ ] 右键菜单显示删除/分割/复制选项
- [ ] 分割功能在 playhead 处正确切分片段
- [ ] Ctrl+D 复制片段到紧跟位置
- [ ] Ctrl+Z / Ctrl+Shift+Z 撤销/重做
- [ ] Undo/Redo 按钮显示且 disabled 状态正确
- [ ] 音频轨片段在预览时播放声音
- [ ] 文本轨片段在预览窗口显示文字叠加
- [ ] 裁剪手柄可拖拽调整 in/out 点
- [ ] 片段拖拽时吸附到 playhead 和其他片段边界
- [ ] 跨轨道拖拽正常（同类型约束）
- [ ] 轨道静音/锁定状态持久化到后端
- [ ] `pnpm typecheck` 零错误
