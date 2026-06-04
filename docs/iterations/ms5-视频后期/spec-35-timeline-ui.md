# SPEC-35: 单轨时间轴 UI 与裁剪交互

> 对应 MS5 原任务 6（基础时间轴 UI）。构建自研的单轨视频时间轴组件，支持缩略图条背景、裁剪手柄拖拽、播放头同步。

依赖：spec-33（`extract_thumbnail_strip` 生成缩略图条）、spec-34（`useFFmpegProgress` 供操作触发时显示进度）

非目标：
- 多轨时间轴（仅单视频轨）
- 音频波形显示
- 片段编排/拼接 — spec-36
- 字幕/气泡叠加时间轴 — MS6

---

## 1. 架构决策

**自研 div-based 组件**，不引入外部时间轴库。理由：
- 需求仅为单轨裁剪，不是完整 NLE
- 现有 React 时间轴库（如 `react-timeline-editor`）偏重多轨编辑，体积大、API 重
- div + pointer events + CSS transform 足够实现裁剪手柄和播放头

**组件树**：

```
VideoEditorLayout
├── VideoPreview              # <video> 元素 + 基本控制
├── Timeline                  # 容器：水平布局 + 缩放控制
│   ├── TimeScale             # 顶部时间刻度尺
│   ├── ThumbnailStrip        # 缩略图条背景
│   ├── TrimHandles           # 左右裁剪手柄 + 排除区域蒙版
│   └── PlaybackHead          # 播放头垂直线
└── TimelineControls          # 缩放滑块 + 时间显示
```

---

## 2. 缩略图条（ThumbnailStrip）

**数据获取**：组件 mount 时调用 `extract_thumbnail_strip` IPC（spec-33），传入视频路径、间隔（默认 1000ms）、宽度（120px）。

**渲染**：将缩略图按序排列为水平条，通过 CSS `display: flex` 或 `background-image` 拼接。总宽度 = `thumbnailCount * thumbDisplayWidth`，受缩放级别影响。

**缓存**：缩略图存储在项目 `thumbnails/strips/` 下，按 `{asset_id}_{interval}_{width}` 缓存。TanStack Query 管理请求状态，`staleTime: Infinity`（同一视频的缩略图不会变）。

**加载态**：生成缩略图前显示灰色骨架条 + spinner。

---

## 3. 时间刻度（TimeScale）

div-based 刻度尺，位于缩略图条上方。

| 缩放级别 | 主刻度间隔 | 标签格式 |
|---|---|---|
| < 0.5x | 60s | `1:00`, `2:00` |
| 0.5x – 2x | 10s | `0:10`, `0:20` |
| 2x – 5x | 5s | `0:05`, `0:10` |
| 5x – 10x | 1s | `0:01`, `0:02` |
| > 10x | 0.5s | `0:00.5`, `0:01.0` |

主刻度之间插入 4 个次刻度（不带标签，高度为主刻度一半）。

缩放基准：`1x` = 1 像素 / 100ms。缩放范围 `0.1x – 20x`。

---

## 4. 裁剪手柄（TrimHandles）

两个可拖拽的垂直条（左 = in point，右 = out point）。

**交互**：
- `pointerdown` → 记录初始位置，添加 `pointermove` / `pointerup` 监听
- `pointermove` → 计算 delta，转换为时间偏移量，更新 store
- `pointerup` → 移除监听，触发 `onTrimChange` 回调

**约束**：
- 左手柄：`0 ≤ trimStart ≤ trimEnd - minDuration`
- 右手柄：`trimStart + minDuration ≤ trimEnd ≤ duration`
- `minDuration = 100ms`（防止零长度片段）

**视觉**：
- 手柄：12px 宽、带横向纹理的拖拽把手
- 排除区域（trimStart 之前和 trimEnd 之后）：半透明黑色蒙版（`rgba(0,0,0,0.5)`）
- 选中区域：无蒙版，显示边框高亮

**吸附**：拖拽接近整秒时吸附（±50ms 内吸附到整秒）。可按住 Shift 禁用吸附实现精确定位。

---

## 5. 播放头（PlaybackHead）

垂直红色细线（2px），顶部带倒三角标记。

**双向同步**：
- **视频 → 播放头**：video 播放时通过 `requestAnimationFrame` 轮询 `video.currentTime`，更新播放头位置。每帧一次，约 16ms 精度。
- **播放头 → 视频**：用户在时间轴上点击或拖拽播放头时，设置 `video.currentTime = clickedTime`。

**交互**：
- 点击时间轴空白区域：播放头跳到点击位置
- 拖拽播放头：实时 scrub 视频（`video.currentTime` 跟随）

---

## 6. 视频预览（VideoPreview）

```tsx
<video
  ref={videoRef}
  src={convertFileSrc(videoPath)}
  controls={false}
/>
```

使用 Tauri `convertFileSrc()` 通过 asset protocol 访问本地视频文件。

**自定义控制**：
- 播放/暂停按钮
- 当前时间 / 总时长显示
- 音量控制（可选，MVP 可省略）

不做全屏播放（桌面应用内预览足够）。

---

## 7. 缩放控制（TimelineControls）

- shadcn `Slider` 组件控制缩放级别（0.1x – 20x，对数刻度）
- 滚轮缩放：`Ctrl/Cmd + wheel` 以鼠标位置为锚点缩放
- 缩放时保持播放头在视口中的相对位置不变
- 当前播放时间 + 裁剪范围时间的文字显示

---

## 8. Zustand Store

```typescript
// src/stores/timelineStore.ts

interface TimelineState {
    videoSrc: string | null;
    duration: number;           // ms
    trimStart: number;          // ms
    trimEnd: number;            // ms
    playbackPosition: number;   // ms
    zoom: number;               // multiplier, default 1.0
    isPlaying: boolean;

    setVideoSrc: (src: string, duration: number) => void;
    setTrim: (start: number, end: number) => void;
    setPlaybackPosition: (pos: number) => void;
    setZoom: (zoom: number) => void;
    togglePlay: () => void;
}
```

**不持久化到 DB**——纯 session 状态，关闭编辑器时丢弃。裁剪参数在「添加到片段列表」时才写入 `video_clip` 表（spec-36）。

---

## 9. 目录增量

```
src/components/editor/
├── VideoEditorLayout.tsx    # 整体布局容器
├── VideoPreview.tsx         # <video> 预览 + 自定义控制
├── Timeline.tsx             # 时间轴容器（处理水平滚动 + 缩放变换）
├── TimeScale.tsx            # 时间刻度尺
├── ThumbnailStrip.tsx       # 缩略图条背景
├── TrimHandles.tsx          # 左右裁剪手柄 + 排除区域蒙版
├── PlaybackHead.tsx         # 播放头
└── TimelineControls.tsx     # 缩放滑块 + 时间显示

src/stores/
└── timelineStore.ts         # Zustand store

src/hooks/
└── useThumbnailStrip.ts     # TanStack Query wrapper for extract_thumbnail_strip
```

---

## 10. 测试

| 用例 | 说明 |
|---|---|
| `TimeScale renders ticks` | 不同缩放级别下刻度间隔正确 |
| `TrimHandles constrained` | 左手柄不超过右手柄，右手柄不超过时长 |
| `TrimHandles snap` | 拖拽到 4950ms 吸附到 5000ms |
| `TrimHandles shift_no_snap` | 按住 Shift 时不吸附 |
| `PlaybackHead position` | position=5000ms 在 zoom=1x 时 left=50px |
| `timelineStore setTrim` | store 状态更新正确 |
| `zoom levels` | 缩放级别约束在 0.1–20 范围内 |
| `ThumbnailStrip loading` | 加载中显示骨架条 |

---

## 11. 验收清单

- [ ] 时间轴正确显示视频时长对应的缩略图条
- [ ] 缩略图条加载有 loading 态，生成完成后正确渲染
- [ ] 可拖动左右裁剪手柄设置裁剪范围
- [ ] 手柄有最小间距约束（不能产生 <100ms 的片段）
- [ ] 拖拽手柄时有整秒吸附，Shift 取消吸附
- [ ] 裁剪排除区域显示半透明蒙版
- [ ] 时间刻度随缩放级别自适应（标签间隔、格式变化）
- [ ] 播放头位置与视频播放进度同步
- [ ] 点击时间轴空白区域跳转播放位置
- [ ] 拖拽播放头实时 scrub 视频画面
- [ ] Ctrl/Cmd + 滚轮缩放时间轴
