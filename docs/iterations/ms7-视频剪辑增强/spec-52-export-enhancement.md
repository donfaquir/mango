# SPEC-52: 导出增强

> 对应 MS7 任务 8。增强导出功能，支持宽高比变换、平台预设、编码参数可配置 UI、封面帧提取。

依赖：
- spec-45（`export_timeline()` + `RenderConfig` 已实现，ExportSettingsSheet 已存在）
- spec-42（`FilterGraph::scale()`, `pad()`, `format()` 方法已存在）
- spec-43/44（多轨时间轴数据 + UI 已落地）

非目标：
- 批量导出（多集同时导出）— 产品价值需进一步验证，延后
- 封面帧自动推荐（图像分析评分）— 算法复杂度高，首版仅手动选取
- WebM/AV1 输出格式 — 首版仅 MP4 (H.264/H.265)

---

## 1. 已有基础

### 1.1 RenderConfig

```rust
pub struct RenderConfig {
    pub video_codec: String,     // "libx264"
    pub preset: String,          // "fast"
    pub crf: u32,                // 18
    pub audio_bitrate: String,   // "128k"
    pub container: String,       // "mp4"
    pub output_width: Option<i32>,
    pub output_height: Option<i32>,
    pub output_fps: Option<f64>,
}
```

### 1.2 ExportSettingsSheet

当前 UI（108 行）仅显示输出路径和进度条。RenderConfig 全部硬编码：
```typescript
{ video_codec: "libx264", preset: "fast", crf: 18, ... }
```

### 1.3 normalize 管线

`build_filter_graph()` 第 1 阶段已实现 clip 归一化（scale + fps + format），使用 `config.output_width/height/fps`。当这些值为 None 时取第一个 clip 的参数。

---

## 2. 宽高比变换

### 2.1 预设定义

**新建**：`crates/core/src/aspect_ratio.rs`（~80 行）

```rust
pub struct AspectRatioPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub width: u32,
    pub height: u32,
    pub fit_mode: FitMode,
}

pub enum FitMode {
    Crop,    // 裁剪适配（无黑边，可能丢失画面）
    Pad,     // 填充适配（保留全部画面，加黑边/模糊背景）
    Stretch, // 拉伸适配（变形）
}
```

预设列表：

| ID | 标签 | 分辨率 | 用途 |
|---|---|---|---|
| `16_9` | 16:9 横屏 | 1920×1080 | B站/YouTube |
| `9_16` | 9:16 竖屏 | 1080×1920 | 抖音/TikTok |
| `1_1` | 1:1 方形 | 1080×1080 | 微信/小红书 |
| `3_4` | 3:4 竖版 | 1080×1440 | 小红书 |
| `custom` | 自定义 | 用户输入 | — |

### 2.2 渲染管线适配

**文件**：`crates/core/src/ffmpeg/render.rs`

RenderConfig 的 `output_width/height` 已支持指定输出分辨率。normalize 阶段的 `scale` filter 已将所有 clip 统一到目标分辨率。

需补充：当源 clip 与目标宽高比不同时的处理：

**Crop 模式**：在 normalize 中追加 `crop` filter
```
scale=max(W,iw*H/ih):max(H,ih*W/iw),crop=W:H
```

**Pad 模式**：在 normalize 中使用 `scale+pad` 组合
```
scale=min(W,iw*H/ih):min(H,ih*W/iw),pad=W:H:(W-iw)/2:(H-ih)/2:black
```

### 2.3 扩展 FilterGraph

**文件**：`crates/core/src/ffmpeg/filter_graph.rs`

新增 `crop` 方法：

```rust
pub fn crop(&mut self, input: &str, width: u32, height: u32, x: &str, y: &str, output: &str)
```

### 2.4 扩展 normalize

**文件**：`crates/core/src/ffmpeg/render.rs`

修改 normalize 阶段，根据 `FitMode` 选择 crop 或 pad 策略。RenderConfig 需新增 `fit_mode` 字段：

```rust
pub struct RenderConfig {
    // existing fields...
    pub fit_mode: Option<String>,  // NEW: "crop" | "pad" | "stretch", default "pad"
}
```

---

## 3. 平台预设

### 3.1 定义

**文件**：`crates/core/src/aspect_ratio.rs`（追加）

```rust
pub struct PlatformPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub aspect: AspectRatioPreset,
    pub render_config: RenderConfig,
}
```

| ID | 标签 | 宽高比 | 编码 |
|---|---|---|---|
| `douyin` | 抖音 | 9:16, 1080×1920 | H.264, preset=fast, crf=18 |
| `bilibili` | B站 | 16:9, 1920×1080 | H.264, preset=fast, crf=18 |
| `wechat_h` | 微信横版 | 16:9, 1920×1080 | H.264, preset=fast, crf=20 |
| `wechat_v` | 微信竖版 | 9:16, 1080×1920 | H.264, preset=fast, crf=20 |
| `xiaohongshu` | 小红书 | 3:4, 1080×1440 | H.264, preset=fast, crf=18 |
| `custom` | 自定义 | 用户指定 | 用户指定 |

选择平台预设时自动填充 RenderConfig 的所有字段。

---

## 4. 封面帧提取

### 4.1 后端

**文件**：`crates/core/src/ffmpeg/thumbnail.rs`（已存在，追加）

新增函数：

```rust
pub fn extract_frame_at(
    config: &FfmpegConfig,
    input: &Path,
    time_ms: i64,
    output: &Path,
) -> Result<PathBuf>
```

FFmpeg 命令：`ffmpeg -ss {time_s} -i input -frames:v 1 -q:v 2 output.jpg`

### 4.2 Tauri 命令

**文件**：`src-tauri/src/commands/ffmpeg.rs`

新增 `extract_cover_frame` 命令：

```rust
pub async fn extract_cover_frame(
    input: String,
    time_ms: i64,
    output: String,
) -> Result<String, IpcError>
```

### 4.3 前端

封面帧选取集成在 ExportSettingsSheet 中：

- 显示视频预览缩略图
- 时间轴上选择封面帧位置（复用 playhead 或独立滑块）
- "提取封面" 按钮 → 调用 `extractCoverFrame` → 保存为 JPG
- 封面路径显示在 UI 中

---

## 5. 前端 ExportSettingsSheet 重构

### 5.1 布局

**文件**：`src/components/editor/ExportSettingsSheet.tsx`

从当前 108 行扩展为 ~200 行，新增以下 UI 区域：

```
┌──────────────────────┐
│ 平台预设选择         │  ← 6 个平台按钮
├──────────────────────┤
│ 宽高比               │  ← 5 个比例按钮 + 适配模式
├──────────────────────┤
│ 编码设置             │  ← codec/preset/CRF 控件
├──────────────────────┤
│ 封面帧               │  ← 缩略图 + 提取按钮
├──────────────────────┤
│ 输出路径             │  ← 现有路径输入
├──────────────────────┤
│ 进度条               │  ← 现有进度条
├──────────────────────┤
│ 导出按钮             │
└──────────────────────┘
```

### 5.2 编码设置控件

| 控件 | 类型 | 选项 |
|------|------|------|
| 编码器 | Select | H.264 / H.265 |
| 编码速度 | Select | ultrafast / fast / medium / slow |
| 画质 (CRF) | Slider (0-51) | 默认 18，显示数值 |
| 分辨率 | Select | 720p / 1080p / 1440p / 原始 |
| 帧率 | Select | 24fps / 30fps / 60fps / 原始 |

选择平台预设时自动填充以上控件。用户可手动覆盖。

### 5.3 状态管理

将 RenderConfig 从硬编码改为 `useState`，各控件绑定 state：

```typescript
const [renderConfig, setRenderConfig] = useState<RenderConfig>({
  video_codec: "libx264",
  preset: "fast",
  crf: 18,
  audio_bitrate: "128k",
  container: "mp4",
  output_width: null,
  output_height: null,
  output_fps: null,
  fit_mode: "pad",
});
```

---

## 6. CLI 扩展

**文件**：`crates/cli/src/commands/export.rs`

扩展 `mango export timeline` 子命令参数：

```
--width <W>          输出宽度
--height <H>         输出高度
--fit-mode <MODE>    crop | pad | stretch
--platform <ID>      平台预设 (douyin | bilibili | xiaohongshu | ...)
--cover-time <MS>    封面帧时间点
--cover-output <PATH> 封面帧输出路径
```

`--platform` 参数自动设置 width/height/codec/preset/crf。

---

## 7. 目录增量

### 新建文件

| 文件 | 行数（估） | 职责 |
|------|-----------|------|
| `crates/core/src/aspect_ratio.rs` | ~80 | 宽高比预设 + 平台预设 |

### 修改文件

| 文件 | 变更 |
|------|------|
| `crates/core/src/lib.rs` | `pub mod aspect_ratio;` |
| `crates/core/src/ffmpeg/filter_graph.rs` | +crop 方法 |
| `crates/core/src/ffmpeg/render.rs` | RenderConfig +fit_mode, normalize 支持 crop/pad |
| `crates/core/src/ffmpeg/thumbnail.rs` | +extract_frame_at 函数 |
| `src-tauri/src/commands/ffmpeg.rs` | +extract_cover_frame 命令 |
| `src-tauri/src/lib.rs` | 注册新命令 |
| `src/components/editor/ExportSettingsSheet.tsx` | 重构为完整导出设置面板 |
| `crates/cli/src/commands/export.rs` | 扩展 CLI 参数 |

---

## 8. 测试

| 用例 | 说明 |
|------|------|
| `aspect_ratio_presets_count` | 5 个宽高比预设 |
| `platform_presets_count` | 6 个平台预设 |
| `platform_douyin_config` | 抖音预设分辨率为 1080×1920 |
| `crop_filter_expression` | crop 模式生成正确的 filter chain |
| `pad_filter_expression` | pad 模式生成正确的 filter chain |
| `render_config_with_fit_mode` | fit_mode 正确传递到 normalize |
| `extract_frame_at_basic` | 指定时间点提取帧 |
| `render_config_default_fit_mode` | 默认 fit_mode 为 "pad" |

---

## 9. 验收清单

- [ ] ExportSettingsSheet 显示平台预设选择器
- [ ] 选择平台预设后自动填充编码参数
- [ ] 5 种宽高比预设可选择
- [ ] Crop 和 Pad 两种适配模式在导出中效果正确
- [ ] 编码器（H.264/H.265）、速度、CRF 可在 UI 调整
- [ ] CRF 调整后导出文件质量/大小有明显变化
- [ ] 分辨率和帧率可在 UI 调整
- [ ] 封面帧可选取并导出为 JPG 图片
- [ ] CLI `--platform douyin` 参数正确设置输出参数
- [ ] 导出视频在主流播放器中正常播放
- [ ] `cargo test -p mango-core` 相关测试全通过
- [ ] `pnpm typecheck` 零错误
