# SPEC-44: 多轨时间轴前端 UI + Store + 代理预览

> 对应 MS7 任务 3 前端部分。构建多轨时间轴编辑器 UI、Zustand store（含 undo/redo）、FFmpeg 低分辨率代理预览。

依赖：spec-43（timeline 数据模型 + IPC 命令）

非目标：
- 数据库 schema 和后端查询 — spec-43
- FFmpeg filter graph 扩展 — spec-42
- 导出管线 — spec-45
- 关键帧编辑 UI — 任务 4
- 转场选择器 UI — 任务 5
- 文字编辑面板 — 任务 6

---

## 1. 替代范围

本 spec 替代当前编辑器中的两套独立组件，统一为一个多轨时间轴。

### 1.1 废弃组件（任务完成后删除）

| 文件 | 行数 | 替代方式 |
|---|---|---|
| `Timeline.tsx` | 71 | → `MultiTrackTimeline.tsx` |
| `ClipAssembly.tsx` | 135 | clip 列表融入视频轨 |
| `ClipCard.tsx` | 63 | → `TimelineItem.tsx` clip 渲染 |
| `TrimHandles.tsx` | 120 | → item 的 in/out handle |
| `VideoEditorLayout.tsx` | 66 | → `MultiTrackEditor.tsx` |
| `timelineStore.ts` | 54 | → `multiTrackStore.ts` |

### 1.2 保留复用（无需修改）

| 文件 | 行数 | 原因 |
|---|---|---|
| `TimeScale.tsx` | 82 | 接口 `{ duration, zoom }` 与多轨兼容 |
| `PlaybackHead.tsx` | 24 | 接口 `{ positionMs, zoom }` 与多轨兼容 |
| `timelineUtils.ts` | 7 | `msToPixel()` / `pixelToMs()` 通用 |
| `ThumbnailStrip.tsx` | 52 | 在 `TimelineItem.tsx` 中复用其逻辑 |
| `VideoPreview.tsx` | 86 | 播放代理预览文件 |

### 1.3 需重写

| 文件 | 行数 | 原因 |
|---|---|---|
| `TimelineControls.tsx` | 60 | trimStart/trimEnd 接口不适用多轨 → `MultiTrackControls.tsx` |
| `ExportSettingsSheet.tsx` | 141 | 追加 timeline 导出入口（spec-45 时修改） |

---

## 2. Zustand Store

### 2.1 `src/stores/multiTrackStore.ts`

```typescript
interface MultiTrackState {
  // === Data (tracked by undo/redo) ===
  tracks: TimelineTrack[];
  items: Record<string, TimelineItem>;  // keyed by item.id

  // === UI state (NOT tracked by undo/redo) ===
  selection: Set<string>;   // selected item IDs
  playhead: number;         // ms
  zoom: number;             // pixels per 100ms
  scrollX: number;          // horizontal scroll offset
  isPlaying: boolean;
  proxyState: 'idle' | 'rendering' | 'ready' | 'stale';
  proxyPath: string | null;

  // === Actions ===
  init(episodeId: string): Promise<void>;
  reset(): void;

  // Track CRUD
  addTrack(input: CreateTimelineTrackInput): Promise<void>;
  removeTrack(trackId: string): Promise<void>;
  reorderTracks(trackIds: string[]): Promise<void>;
  toggleMuted(trackId: string): void;
  toggleLocked(trackId: string): void;

  // Item CRUD
  addItem(input: CreateTimelineItemInput): Promise<void>;
  updateItem(id: string, input: UpdateTimelineItemInput): Promise<void>;
  moveItem(id: string, trackId: string, positionMs: number): Promise<void>;
  removeItem(id: string): Promise<void>;

  // Selection
  selectItem(id: string, multi?: boolean): void;
  deselectAll(): void;

  // Playback
  setPlayhead(ms: number): void;
  setZoom(zoom: number): void;
  setScrollX(x: number): void;

  // Preview
  renderPreview(): Promise<void>;
  invalidateProxy(): void;
}
```

### 2.2 zundo 集成

复用 `canvasStore.ts`（230 行）的模式：

```typescript
export const useMultiTrackStore = create<MultiTrackState>()(
  temporal(
    (set, get) => ({ /* ... */ }),
    {
      limit: 50,
      partialize: (state) => ({
        tracks: state.tracks,
        items: state.items,
      }),
      equality: (a, b) =>
        a.tracks === b.tracks && a.items === b.items,
    },
  ),
);
```

关键行为：
- `partialize`：仅追踪 `tracks` + `items`（playhead/zoom/selection/proxy 不进 undo 栈）
- `init()` / `reset()` 中 pause → set → resume → clear（防止程序化加载进入 undo 栈）
- `addItem` / `moveItem` / `removeItem` 等 action 先更新 store，然后 async 调用 IPC 持久化

### 2.3 Auto-save

以 1s debounce 将变更持久化到 DB（复用画布的 auto-save 模式）。监听 `tracks` 和 `items` 的变更，debounce 后批量写入。

### 2.4 默认轨道初始化

`init(episodeId)` 流程：
1. 调用 `commands.listTimelineTracks(episodeId)`
2. 若返回空数组 → 调用 `commands.createDefaultTracks(episodeId)` 创建 6 条默认轨道
3. 加载所有轨道的 items
4. pause/set/resume/clear（不进 undo 栈）

---

## 3. UI 组件

### 3.1 组件树

```
VideoEditorPage.tsx (修改)
└── MultiTrackEditor.tsx (NEW, 替代 VideoEditorLayout.tsx)
    ├── VideoPreview.tsx (保留)
    ├── MultiTrackTimeline.tsx (NEW)
    │   ├── TimeScale.tsx (保留)
    │   ├── PlaybackHead.tsx (保留)
    │   ├── TrackHeader.tsx (NEW) × N
    │   └── TrackLane.tsx (NEW) × N
    │       └── TimelineItem.tsx (NEW) × M
    └── MultiTrackControls.tsx (NEW)
```

### 3.2 `MultiTrackTimeline.tsx`

主时间轴组件，替代 `Timeline.tsx` + `ClipAssembly.tsx`。

布局：
- 左侧固定区域（~200px）：`TrackHeader` 列表（轨道名称、mute/lock 按钮、类型图标）
- 右侧可滚动区域：水平时间轴，每条轨道一行
- 顶部：`TimeScale` 时间刻度尺
- 浮动：`PlaybackHead` 播放头指示器

交互：
- 点击轨道空白处：设置 playhead 位置
- 双击轨道空白处：在该位置添加 item
- item 拖拽：`@dnd-kit` 水平移动（改 position_ms）
- item 跨轨道拖拽：垂直移动到其他轨道（改 track_id + position_ms）
- item 左右 handle：拖拽调整 in/out point
- Ctrl+滚轮：缩放（复用现有 Timeline.tsx 的模式）

### 3.3 `TimelineItem.tsx`

单个 item 的可视化，按 `item_type` 分支渲染：
- **clip**：显示缩略图条（复用 `ThumbnailStrip.tsx` 的 `useThumbnailStrip` hook）
- **text**：显示文字内容预览 + 类型图标
- **transition**：显示转场类型图标
- **sticker/effect**：显示类型图标 + 颜色条

选中态显示蓝色边框，可拖拽 handle（左=in_point, 右=out_point）。

### 3.4 `TrackHeader.tsx`

轨道标签区域：
- 轨道名称（可编辑）
- 轨道类型图标（Video/Audio/Text/Overlay）
- Mute 按钮（切换静音）
- Lock 按钮（切换锁定，锁定时禁止编辑）

### 3.5 `MultiTrackControls.tsx`

替代 `TimelineControls.tsx`（60 行）：
- 播放/暂停按钮（触发代理渲染 + 播放）
- 播放头时间显示（mm:ss.ms）
- Zoom slider（复用现有 zoom 逻辑）
- 总时长显示
- 删除：trimStart/trimEnd 显示（多轨无全局 trim 概念）

### 3.6 性能优化

- **虚拟滚动**：仅渲染可视区域内的 items（基于 scroll position + zoom 计算可见时间范围）
- **懒加载缩略图**：clip item 进入视口后才请求 `extractThumbnailStrip`
- **大量 item 场景**：100+ items 时滚动流畅

---

## 4. 代理预览

### 4.1 机制

- 用户点击播放按钮 → `renderPreview()` action
- 调用后端（复用 render.rs 的 `render_timeline`，传 480p + ultrafast + crf 28）
- 代理文件缓存在 temp 目录（`<temp>/mango_proxy_<episode_id>.mp4`）
- 渲染完成 → 设置 `proxyPath`，`proxyState = 'ready'`
- `VideoPreview.tsx` 播放代理文件
- 任何 timeline 编辑 → `invalidateProxy()`，`proxyState = 'stale'`

### 4.2 进度显示

复用 `FfmpegProgressTick` 事件机制：
- 渲染进度在 `MultiTrackControls` 中显示进度条
- 渲染期间播放按钮显示 spinner

### 4.3 Tauri IPC

新增 `render_preview` 命令（在 `commands/timeline.rs` 中）：
- 签名：`(episode_id) -> String`（返回代理文件路径）
- 内部：读 timeline 数据 → 调用 `render_timeline(low_res_config, ...)` → 返回路径

---

## 5. 页面布局变更

### 5.1 `VideoEditorPage.tsx` 修改

当前布局（竖向堆叠）：
```
Header → VideoEditorLayout → ClipAssembly → ExportSettingsSheet
```

新布局：
```
Header → MultiTrackEditor → ExportSettingsSheet
```

`MultiTrackEditor` 内部布局：
```
┌─────────────────────────────┐
│       VideoPreview          │
│       (proxy playback)      │
├─────────────────────────────┤
│ TrackHeaders │ Timeline     │
│              │ (scrollable) │
│              │              │
├──────────────┴──────────────┤
│     MultiTrackControls      │
└─────────────────────────────┘
```

### 5.2 TanStack Query hooks

新建 `src/hooks/useTimeline.ts`：

```typescript
export function useTimelineTracks(episodeId: string)
export function useTimelineItems(trackId: string)
export function useImportAudio(episodeId: string)
```

这些 hooks 主要用于初始加载，运行时数据由 `multiTrackStore` 管理。

---

## 6. 目录增量

```
src/
├── stores/
│   └── multiTrackStore.ts              # NEW
├── hooks/
│   └── useTimeline.ts                  # NEW
└── components/editor/
    ├── MultiTrackEditor.tsx            # NEW (替代 VideoEditorLayout.tsx)
    ├── MultiTrackTimeline.tsx          # NEW (替代 Timeline.tsx + ClipAssembly.tsx)
    ├── TimelineItem.tsx                # NEW (替代 ClipCard.tsx)
    ├── TrackHeader.tsx                 # NEW
    ├── MultiTrackControls.tsx          # NEW (替代 TimelineControls.tsx)
    ├── TimeScale.tsx                   # 保留
    ├── PlaybackHead.tsx                # 保留
    ├── ThumbnailStrip.tsx              # 保留
    ├── VideoPreview.tsx                # 保留
    ├── ExportSettingsSheet.tsx         # 保留（spec-45 时修改）
    └── timelineUtils.ts               # 保留
```

废弃文件在多轨编辑器功能验证后删除：
`Timeline.tsx`, `ClipAssembly.tsx`, `ClipCard.tsx`, `TrimHandles.tsx`, `VideoEditorLayout.tsx`, `timelineStore.ts`

---

## 7. 测试

| 用例 | 说明 |
|------|------|
| `store_init_creates_defaults` | 空 episode init 后创建 6 条默认轨道 |
| `store_init_loads_existing` | 已有轨道的 episode 正确加载 |
| `store_init_no_undo` | init 不进 undo 栈（pause/resume/clear） |
| `store_add_item_undo` | addItem 后 undo 回退，redo 恢复 |
| `store_move_item_undo` | moveItem 后 undo 回退 |
| `store_viewport_no_undo` | zoom/scroll/playhead 变更不进 undo 栈 |
| `store_undo_stack_limit` | 栈上限 50 |
| `store_invalidate_proxy` | 编辑操作后 proxyState 变为 stale |
| `timeline_renders_tracks` | MultiTrackTimeline 渲染正确数量的轨道行 |
| `timeline_item_clip_thumbnail` | clip 类型 item 渲染缩略图 |
| `track_header_mute_toggle` | 点击 mute 按钮切换状态 |
| `controls_play_button` | 播放按钮触发 renderPreview |

---

## 8. 验收清单

- [ ] 多轨编辑器正确替代原有 Timeline + ClipAssembly
- [ ] 6 条默认轨道在首次进入编辑器时自动创建
- [ ] 可在轨道上添加、移动、调整 in/out point、删除 item
- [ ] item 可跨轨道拖拽
- [ ] Undo/Redo 对所有 track/item 操作生效
- [ ] viewport（zoom/scroll/playhead）不进 undo 栈
- [ ] 时间轴数据 auto-save 到 DB（1s debounce）
- [ ] 播放按钮触发代理渲染 → 显示进度 → 完成后播放预览
- [ ] timeline 编辑后 proxy 标记为 stale
- [ ] 100+ item 场景下时间轴滚动流畅
- [ ] `pnpm typecheck` 零错误
- [ ] 前端测试（store + 组件）通过
