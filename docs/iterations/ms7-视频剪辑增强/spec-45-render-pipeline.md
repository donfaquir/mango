# SPEC-45: 重编码导出管线 + export_timeline

> 对应 MS7 任务 2（完整）。新增基于 filter graph 的重编码导出管线，将多轨时间轴数据渲染为最终视频。保留 MS5/MS6 的简单 Copy 模式导出不变。

依赖：spec-42（filter graph + render.rs）、spec-43（timeline 数据模型）

非目标：
- filter graph 的 video filter 方法 — spec-42
- timeline 数据库表和查询 — spec-43
- 前端多轨 UI — spec-44
- 宽高比预设/平台预设/封面帧 — 任务 8

---

## 1. 向后兼容

### 1.1 保持不变的函数和命令

以下函数和 Tauri 命令保持原样，不做任何修改：

| 组件 | 数据源 | 用途 |
|---|---|---|
| `export.rs::resolve_clips()` | `video_clip` 表 | MS5 简单 clip resolve |
| `export.rs::export_resolved_clips()` | ResolvedClip 列表 | MS5 视频拼接 |
| `export.rs::resolve_final_export()` | `video_clip` + `shot_audio` 表 | MS6 音频 resolve |
| `export.rs::export_final()` | ResolvedFinalExport | MS6 带音频导出 |
| `commands/video.rs::export_video_clips` | IPC → resolve_clips → export | Tauri 命令 |
| `commands/video.rs::export_final` | IPC → resolve_final_export → export_final | Tauri 命令 |
| CLI `mango export clip` | — | CLI 命令 |
| CLI `mango export final` | — | CLI 命令 |

---

## 2. 扩展 probe.rs

在 `VideoMetadata` 结构体中新增 `pixel_format` 字段：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export)]
pub struct VideoMetadata {
    pub duration_ms: i64,
    pub width: u32,
    pub height: u32,
    pub video_codec: String,
    pub audio_codec: Option<String>,
    pub fps: f64,
    pub bitrate_kbps: Option<u32>,
    pub file_size_bytes: u64,
    pub pixel_format: Option<String>,  // NEW: e.g. "yuv420p", "yuv444p"
}
```

从 ffprobe JSON 的 `streams[video].pix_fmt` 字段解析。用于 normalize 步骤决定是否需要像素格式转换。

---

## 3. ExportMode 枚举

```rust
// crates/core/src/export.rs

#[derive(Debug, Clone)]
pub enum ExportMode {
    Copy,    // -c copy，快速无转码
    Render,  // filter graph 重编码
}
```

不对外暴露为 IPC 类型——由 `export_timeline` 内部自动判断。

---

## 4. resolve_timeline 函数

```rust
/// Resolve timeline data from DB into a renderable structure.
pub fn resolve_timeline(
    conn: &Connection,
    episode_id: &str,
    workspace_root: &Path,
    ffmpeg_config: &FfmpegConfig,
) -> Result<ResolvedTimeline>
```

流程：
1. 读 `timeline_track` + `timeline_item` 表
2. 按 track_type 分类处理：
   - video track → ResolvedTimelineClip 列表
   - text track → ResolvedTextOverlay 列表
   - overlay track → ResolvedStickerOverlay 列表
   - audio tracks → AudioMixInput 列表
3. 解析每个 item 的 `params_json` → `TimelineItemParams` enum
4. 识别 transition items → ResolvedTransition 列表
5. resolve asset_id → 绝对文件路径
6. probe 各 clip 获取时长/分辨率信息
7. 组装 `ResolvedTimeline`

---

## 5. export_timeline 函数

```rust
/// Export a multi-track timeline to a video file.
pub fn export_timeline(
    conn: &Connection,
    episode_id: &str,
    workspace_root: &Path,
    render_config: &RenderConfig,
    ffmpeg_config: &FfmpegConfig,
    on_progress: Option<&mut dyn FnMut(FfmpegProgress)>,
) -> Result<PathBuf>
```

流程：
1. `resolve_timeline()` 获取 `ResolvedTimeline`
2. **智能降级判断**：
   - 条件：所有 item 都是 clip（无 text/sticker/effect/transition），无 ken_burns，无 color_effect
   - 且所有 clip 同分辨率/帧率/编码器（类似 `check_concat_compatibility` 的检查）
   - → 使用 Copy 模式：复用现有 `concat_videos()` 快速拼接
3. 否则 → Render 模式：调用 `render_timeline()` from `render.rs`

### 5.1 归一化管线（Render 模式内部）

在进入 filter graph 前，对每个输入 clip 执行归一化：
- 目标分辨率：`render_config.output_width/height` 或取第一个 clip 的分辨率
- 目标帧率：`render_config.output_fps` 或 30.0
- 目标像素格式：`yuv420p`（H.264 标准要求）
- 通过 `FilterGraph::normalize()` 方法实现

---

## 6. Tauri IPC 命令

在 `src-tauri/src/commands/video.rs` 中新增：

```rust
#[tauri::command]
#[specta::specta]
pub async fn export_timeline(
    state: State<'_, AppState>,
    app: AppHandle,
    episode_id: String,
    output_path: String,
    render_config: RenderConfig,
) -> Result<String, IpcError>
```

- 内部调用 `crate::export::export_timeline()`
- 进度通过 `FfmpegProgressTick` 事件推送（复用现有模式）
- 返回输出文件路径

---

## 7. CLI 子命令

在 `crates/cli/src/commands/export.rs` 中新增 `timeline` 子命令：

```
mango export timeline --episode <id> --output <path> [--preset fast] [--crf 18] [--codec libx264]
```

参数映射到 `RenderConfig` 各字段。

---

## 8. 前端集成

### 8.1 `ExportSettingsSheet.tsx` 修改

在现有导出设置 sheet 中追加 "时间轴导出" 模式：

- 新增 RenderConfig 设置项：
  - Preset 选择：ultrafast / fast / medium / slow
  - CRF 滑块：0-51（默认 18）
  - Codec 选择：H.264 / H.265
- 新增"时间轴导出"按钮，调用 `commands.exportTimeline()`
- 保留现有"简单导出"按钮（调用 `exportFinal`）

### 8.2 进度显示

复用现有 `useFFmpegProgress` hook 和进度条 UI。

---

## 9. 目录增量

```
crates/core/src/
├── export.rs                # 修改：追加 ExportMode、resolve_timeline、export_timeline
└── ffmpeg/
    └── probe.rs             # 修改：VideoMetadata 追加 pixel_format

src-tauri/src/commands/
└── video.rs                 # 修改：追加 export_timeline 命令

crates/cli/src/commands/
└── export.rs                # 修改：追加 timeline 子命令

src/components/editor/
└── ExportSettingsSheet.tsx   # 修改：追加 RenderConfig UI
```

---

## 10. 测试

| 用例 | 说明 |
|------|------|
| `probe_pixel_format` | ffprobe 正确提取 pix_fmt 字段 |
| `probe_pixel_format_missing` | 无 pix_fmt 时返回 None |
| `resolve_timeline_empty` | 空 timeline 返回 ValidationError |
| `resolve_timeline_clips_only` | 纯 clip timeline 正确 resolve |
| `resolve_timeline_with_audio` | 含音频 item 的 timeline 正确 resolve |
| `export_mode_auto_copy` | 纯 clip + 同参数 → Copy 模式 |
| `export_mode_auto_render` | 有 transition → Render 模式 |
| `export_mode_auto_render_mixed_res` | 不同分辨率 clip → Render 模式 |
| `existing_export_clips_unchanged` | export_resolved_clips API 不变（回归） |
| `existing_export_final_unchanged` | export_final API 不变（回归） |
| `render_config_default` | Default 合理 |

---

## 11. 验收清单

- [ ] 现有 `export_video_clips` / `export_final` 行为不变（回归）
- [ ] `VideoMetadata.pixel_format` 对 mp4/webm 正确返回
- [ ] `resolve_timeline` 正确读 timeline_* 表并 resolve
- [ ] 纯 clip 无特效时间轴 → 自动 Copy 模式，速度与 MS5 一致
- [ ] 有转场/特效时间轴 → Render 模式，输出正确合成视频
- [ ] 不同分辨率 clip → normalize 统一后正确拼接
- [ ] CRF / preset / codec 参数可配置且生效
- [ ] 进度条在 Render 模式下正常工作
- [ ] 输出文件可在主流播放器播放
- [ ] ExportSettingsSheet 增加 timeline 导出模式
- [ ] CLI `mango export timeline` 可用
- [ ] `cargo test -p mango-core` 新增 11+ 测试全通过
