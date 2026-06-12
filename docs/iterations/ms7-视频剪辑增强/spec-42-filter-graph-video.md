# SPEC-42: FFmpeg Filter Graph 视频扩展 + 渲染编排

> 对应 MS7 任务 1（完整）。在现有纯音频 filter graph builder 基础上新增视频 filter 方法，并创建 `render.rs` 渲染编排模块。

依赖：MS5 spec-32（FFmpeg sidecar）、spec-33（video commands）、spec-34（progress parsing）

非目标：
- 多轨时间轴数据模型 — spec-43
- 多轨 UI 和预览 — spec-44
- 导出管线和 export_timeline — spec-45
- 任何前端 UI 变更

---

## 1. 现状与决策

### 1.1 现有 filter_graph.rs（184 行）

`crates/core/src/ffmpeg/filter_graph.rs` 已有：
- `FilterGraph` 结构体：flat `Vec<FilterNode>` 模型
- `FilterNode`（private）：`{ inputs, filter, outputs }` 三元组
- `AmixDuration` 枚举
- 6 个音频 filter 方法：`volume`, `adelay`, `apad`, `atrim`, `amix`, `aconcat`
- `build()` 方法：生成 `;` 分隔的 `-filter_complex` 字符串
- 7 个单元测试

`audio.rs`（380 行）已在 `mix_audio` 和 `overlay_audio_on_video` 中使用 `FilterGraph`。

### 1.2 架构决策

- **保持 flat `Vec<FilterNode>` 模型**：不引入 `FilterChain` 类型。现有模型足够表达线性链和复杂 DAG，`audio.rs` 已依赖此 API。
- **视频 filter 与音频 filter 共存**：同一个 `FilterGraph` 可以同时包含视频和音频 filter 节点，通过 label 前缀区分流（如 `[0:v]` vs `[0:a]`）。
- **`normalize` 便捷方法**：xfade 和 concat 要求所有输入同分辨率/帧率/像素格式。现有 `check_concat_compatibility()`（commands.rs:273）仅检查 `-c copy` 模式兼容性，不检查 pixel format。Render 模式下由 `normalize` 保证。

---

## 2. 扩展 filter_graph.rs

在现有文件中追加视频 filter 方法，保持所有现有 API 不变。

### 2.1 新增方法签名

```rust
// === 视频 filter ===

pub fn scale(&mut self, input: &str, width: i32, height: i32, output: &str) -> &mut Self
// 生成: [input]scale=W:H[output]

pub fn pad(&mut self, input: &str, width: i32, height: i32, x: i32, y: i32, color: &str, output: &str) -> &mut Self
// 生成: [input]pad=W:H:X:Y:color=COLOR[output]

pub fn fps(&mut self, input: &str, fps: f64, output: &str) -> &mut Self
// 生成: [input]fps=FPS[output]

pub fn format(&mut self, input: &str, pix_fmt: &str, output: &str) -> &mut Self
// 生成: [input]format=PIX_FMT[output]

pub fn normalize(&mut self, input: &str, width: i32, height: i32, fps: f64, pix_fmt: &str, output: &str) -> &mut Self
// 便捷组合: scale + fps + format，生成:
// [input]scale=W:H,fps=FPS,format=PIX_FMT[output]
// 注意: 用逗号分隔的单链 filter，非多个独立节点

pub fn zoompan(&mut self, input: &str, zoom_expr: &str, x_expr: &str, y_expr: &str, duration_frames: u32, fps: u32, output: &str) -> &mut Self
// 生成: [input]zoompan=z='ZOOM':x='X':y='Y':d=D:s=WxH:fps=FPS[output]

pub fn xfade(&mut self, input_a: &str, input_b: &str, transition: &str, duration_secs: f64, offset_secs: f64, output: &str) -> &mut Self
// 生成: [input_a][input_b]xfade=transition=TYPE:duration=D:offset=O[output]

pub fn drawtext(&mut self, input: &str, text: &str, fontfile: Option<&str>, fontsize: u32, fontcolor: &str, x: &str, y: &str, enable_expr: Option<&str>, output: &str) -> &mut Self
// 生成: [input]drawtext=text='TEXT':fontfile=FF:fontsize=FS:fontcolor=FC:x=X:y=Y[:enable='EXPR'][output]

pub fn overlay(&mut self, main_input: &str, overlay_input: &str, x: &str, y: &str, enable_expr: Option<&str>, output: &str) -> &mut Self
// 生成: [main][overlay]overlay=x=X:y=Y[:enable='EXPR'][output]

pub fn colorbalance(&mut self, input: &str, rs: f64, gs: f64, bs: f64, rm: f64, gm: f64, bm: f64, rh: f64, gh: f64, bh: f64, output: &str) -> &mut Self
// 生成: [input]colorbalance=rs=RS:gs=GS:bs=BS:rm=RM:gm=GM:bm=BM:rh=RH:gh=GH:bh=BH[output]

pub fn eq(&mut self, input: &str, brightness: f64, contrast: f64, saturation: f64, output: &str) -> &mut Self
// 生成: [input]eq=brightness=B:contrast=C:saturation=S[output]

pub fn video_concat(&mut self, inputs: &[&str], n_segments: usize, has_audio: bool, output_v: &str, output_a: Option<&str>) -> &mut Self
// 生成: [inputs...]concat=n=N:v=1:a=A[output_v][output_a?]
```

### 2.2 normalize 实现要点

`normalize` 生成**逗号分隔的单链 filter**（一个 FilterNode），而非三个独立节点。这是因为 FFmpeg 中 `scale=W:H,fps=F,format=FMT` 在单链中执行更高效。

```rust
pub fn normalize(&mut self, input: &str, width: i32, height: i32, fps: f64, pix_fmt: &str, output: &str) -> &mut Self {
    self.nodes.push(FilterNode {
        inputs: vec![input.to_string()],
        filter: format!("scale={width}:{height},fps={fps},format={pix_fmt}"),
        outputs: vec![output.to_string()],
    });
    self
}
```

### 2.3 drawtext 文本转义

FFmpeg `drawtext` 的 `text` 参数需要特殊转义：冒号 `:` → `\\:`，单引号 `'` → `'\\''`。新增一个 private helper：

```rust
fn escape_drawtext(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace(':', "\\:")
        .replace("'", "'\\''")
}
```

---

## 3. 新建 render.rs

`crates/core/src/ffmpeg/render.rs` — 渲染编排模块，从时间轴数据结构生成 FilterGraph + FFmpeg 命令。

### 3.1 类型定义

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RenderConfig {
    pub video_codec: String,        // "libx264" | "libx265"
    pub preset: String,             // "ultrafast" | "fast" | "medium" | "slow"
    pub crf: u32,                   // 0-51, default 18
    pub audio_bitrate: String,      // "128k"
    pub container: String,          // "mp4" | "webm"
    pub output_width: Option<i32>,  // None = use first clip's resolution
    pub output_height: Option<i32>,
    pub output_fps: Option<f64>,    // None = 30.0
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            video_codec: "libx264".into(),
            preset: "fast".into(),
            crf: 18,
            audio_bitrate: "128k".into(),
            container: "mp4".into(),
            output_width: None,
            output_height: None,
            output_fps: None,
        }
    }
}
```

### 3.2 渲染入口

```rust
/// Render a timeline data structure to an output file.
///
/// `timeline_data` is a structured representation of the multi-track timeline,
/// resolved to absolute file paths and parameters.
pub fn render_timeline(
    config: &RenderConfig,
    ffmpeg_config: &FfmpegConfig,
    timeline_data: &ResolvedTimeline,
    output_path: &Path,
    on_progress: Option<&mut dyn FnMut(FfmpegProgress)>,
) -> Result<PathBuf>
```

### 3.3 ResolvedTimeline 数据结构

```rust
#[derive(Debug)]
pub struct ResolvedTimeline {
    pub clips: Vec<ResolvedTimelineClip>,
    pub transitions: Vec<ResolvedTransition>,
    pub text_overlays: Vec<ResolvedTextOverlay>,
    pub sticker_overlays: Vec<ResolvedStickerOverlay>,
    pub audio_tracks: Vec<AudioMixInput>,
    pub total_duration_ms: i64,
}

#[derive(Debug)]
pub struct ResolvedTimelineClip {
    pub source_path: PathBuf,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub in_point_ms: i64,
    pub out_point_ms: i64,
    pub ken_burns: Option<KenBurnsParams>,
    pub color_effect: Option<ColorEffectParams>,
}

#[derive(Debug)]
pub struct ResolvedTransition {
    pub transition_type: String,
    pub duration_ms: i64,
    pub position_ms: i64,  // where in the timeline
}
```

完整类型定义在实施时随 spec-43 的 `TimelineItemParams` enum 对齐。

### 3.4 渲染流程

1. **probe 所有输入**：获取各 clip 的分辨率/帧率/像素格式
2. **确定输出参数**：取 config 指定值或第一个 clip 的分辨率
3. **为每个 clip 生成 normalize filter**：统一分辨率/帧率/像素格式
4. **应用 per-clip filters**：zoompan（Ken Burns）、colorbalance/eq（调色）
5. **插入 xfade transitions**：在相邻 clip 之间
6. **叠加 overlays**：drawtext（文字）、overlay（贴片）
7. **混合音频**：复用 `audio.rs` 的 filter graph 模式
8. **编码输出**：组装 FFmpeg 命令（-filter_complex + 编码参数），调用 `run_ffmpeg_with_progress`

---

## 4. mod.rs 变更

```rust
// crates/core/src/ffmpeg/mod.rs — 追加:
pub mod render;
```

re-export `RenderConfig`, `ResolvedTimeline`, `render_timeline`。

---

## 5. 目录增量

```
crates/core/src/ffmpeg/
├── mod.rs              # 修改：追加 pub mod render
├── filter_graph.rs     # 修改：新增 11 个视频 filter 方法 + normalize
└── render.rs           # NEW: 渲染编排模块
```

---

## 6. 测试

| 用例 | 说明 |
|------|------|
| `scale_filter` | `scale(640, 480)` 生成 `[in]scale=640:480[out]` |
| `pad_filter` | pad 带 color 参数正确序列化 |
| `normalize_single_chain` | normalize 生成逗号分隔的单链 `scale=W:H,fps=F,format=FMT` |
| `xfade_dissolve` | 两输入 + dissolve 转场生成正确格式 |
| `drawtext_escape` | 文本中含冒号/引号被正确转义 |
| `overlay_with_enable` | enable 表达式正确插入 |
| `zoompan_expr` | zoom/x/y 表达式正确嵌入 |
| `colorbalance_params` | 9 个参数全部正确序列化 |
| `eq_params` | brightness/contrast/saturation 正确序列化 |
| `video_concat_with_audio` | video+audio concat 生成正确 `v=1:a=1` |
| `complex_mixed_graph` | 5+ 输入 + 视频/音频混合，build() 无语法错误 |
| `existing_audio_api_unchanged` | 现有 volume/adelay/amix 测试仍通过（回归） |
| `render_config_default` | Default trait 生成合理默认值 |

---

## 7. 验收清单

- [ ] 现有 6 个音频 filter 方法 API 不变，`audio.rs` 无需修改
- [ ] `FilterGraph::build()` 对 3 个视频输入 + normalize + xfade 生成正确的 `-filter_complex` 字符串
- [ ] `normalize` 方法生成 `[in]scale=W:H,fps=F,format=FMT[out]` 格式的单链
- [ ] `drawtext` 正确转义特殊字符
- [ ] 单元测试覆盖全部 11 个新 filter 方法 + normalize
- [ ] 复杂 graph（5+ 输入、视频+音频混合）生成的命令无语法错误
- [ ] `RenderConfig` 类型定义完成，Default 合理
- [ ] `render_timeline()` 函数签名和 `ResolvedTimeline` 类型定义完成
- [ ] `cargo test -p mango-core` 新增 13+ 测试全通过
- [ ] 现有音频 filter 测试（7 个）回归通过
