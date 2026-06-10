# SPEC-40: FFmpeg 音频处理基础设施

> 新增 spec（原任务 7 扩展）。扩展 ffmpeg 模块，新增 filter_complex 命令构建器和音频操作函数，为最终合成（spec-41）提供底层音频处理能力。

依赖：MS5 spec-32（ffmpeg sidecar 可用）

非目标：
- 音频波形可视化
- 实时音频预览
- 音频特效（混响、回声、均衡器）
- 无损音频编辑
- 视频滤镜（视频 filter graph 属于 MS7）

---

## 1. 设计决策

### 1.1 为什么需要此 spec

当前 ffmpeg 模块（spec-32/33）仅有视频操作（trim、split、concat、thumbnail）。最终合成需要：
- 多路音频混合（配音 + 音效 + BGM）
- 音量调节
- 时间偏移（延迟插入）
- 音频时长对齐（补静音或裁剪）
- 音频覆盖到视频

这些都依赖 FFmpeg 的 `-filter_complex` 能力，现有代码中**零 filter graph 相关实现**。

### 1.2 typed FilterGraph builder

FFmpeg `-filter_complex` 语法易出错（如 `[0:a]volume=0.8[a0];[1:a]adelay=2000|2000[a1];[a0][a1]amix=inputs=2[out]`）。使用 typed builder 避免字符串拼接错误，提供编译期保障。

### 1.3 同步执行模式

与 spec-33 保持一致：所有操作为同步 `std::process::Command`，通过可选 progress callback 报告进度。不引入 async FFmpeg 进程管理。

### 1.4 音画对齐策略

纯计算函数，不自动执行操作。返回建议策略让调用方决定：
- **Exact**：时长差 < tolerance（默认 500ms）
- **PadSilence**：音频短 → 末尾补静音
- **TrimAudio**：音频稍长 → 裁剪末尾
- **RegenerateNeeded**：音频远长于视频 → 建议调整语速重新生成

---

## 2. FilterGraph 构建器

```rust
// crates/core/src/ffmpeg/filter_graph.rs

/// Typed builder for FFmpeg -filter_complex strings
pub struct FilterGraph {
    nodes: Vec<FilterNode>,
}

pub struct FilterNode {
    pub inputs: Vec<String>,     // e.g. ["0:a"], ["a0", "a1"]
    pub filter: String,          // e.g. "volume=0.8", "amix=inputs=2:duration=longest"
    pub outputs: Vec<String>,    // e.g. ["a0"], ["out"]
}

impl FilterGraph {
    pub fn new() -> Self;

    /// Add a volume filter: [input]volume=vol[output]
    pub fn volume(&mut self, input: &str, vol: f64, output: &str) -> &mut Self;

    /// Add a delay filter: [input]adelay=delay_ms|delay_ms[output]
    pub fn adelay(&mut self, input: &str, delay_ms: i64, output: &str) -> &mut Self;

    /// Add pad with silence: [input]apad=whole_dur=duration_ms_as_seconds[output]
    pub fn apad(&mut self, input: &str, duration_ms: i64, output: &str) -> &mut Self;

    /// Trim audio: [input]atrim=end=seconds[output]
    pub fn atrim(&mut self, input: &str, end_ms: i64, output: &str) -> &mut Self;

    /// Mix N inputs: [i0][i1]...[iN]amix=inputs=N:duration=dur[output]
    pub fn amix(&mut self, inputs: &[&str], duration: AmixDuration, output: &str) -> &mut Self;

    /// Concatenate audio segments: [i0][i1]...[iN]concat=n=N:v=0:a=1[output]
    pub fn aconcat(&mut self, inputs: &[&str], output: &str) -> &mut Self;

    /// Generate the filter_complex string
    pub fn build(&self) -> String;

    /// Returns true if no nodes added
    pub fn is_empty(&self) -> bool;
}

#[derive(Debug, Clone, Copy)]
pub enum AmixDuration {
    Longest,
    Shortest,
    First,
}
```

build() 输出示例：
```
[0:a]volume=0.8[a0];[1:a]adelay=2000|2000[a1];[a0][a1]amix=inputs=2:duration=longest[out]
```

---

## 3. 音频操作函数

```rust
// crates/core/src/ffmpeg/audio.rs

/// Mix multiple audio files into one output file
pub fn mix_audio(
    config: &FfmpegConfig,
    inputs: &[AudioMixInput],
    output: &Path,
    total_duration_ms: Option<i64>,
    on_progress: Option<&mut dyn FnMut(FfmpegProgress)>,
) -> Result<PathBuf>;

/// Input descriptor for audio mixing
#[derive(Debug, Clone)]
pub struct AudioMixInput {
    pub path: PathBuf,
    pub volume: f64,         // 0.0 - 2.0, default 1.0
    pub offset_ms: i64,      // delay from start, default 0
}

/// Overlay audio tracks onto a video file
pub fn overlay_audio_on_video(
    config: &FfmpegConfig,
    video: &Path,
    audio_tracks: &[AudioMixInput],
    output: &Path,
    on_progress: Option<&mut dyn FnMut(FfmpegProgress)>,
) -> Result<PathBuf>;

/// Pad audio with silence to reach target duration
pub fn pad_audio_to_duration(
    config: &FfmpegConfig,
    input: &Path,
    target_duration_ms: i64,
    output: &Path,
) -> Result<PathBuf>;

/// Trim audio to a maximum duration
pub fn trim_audio(
    config: &FfmpegConfig,
    input: &Path,
    end_ms: i64,
    output: &Path,
) -> Result<PathBuf>;

/// Get audio duration in milliseconds using ffprobe
pub fn probe_audio_duration(config: &FfmpegConfig, path: &Path) -> Result<i64>;
```

### 3.1 mix_audio 实现逻辑

1. 验证 inputs 非空，volume 范围合法
2. 构建 FilterGraph：每个 input → volume → adelay → amix → output
3. 生成 ffmpeg 命令：`-i file1 -i file2 ... -filter_complex "{graph}" -map "[out]" output`
4. 执行并解析进度

### 3.2 overlay_audio_on_video 实现逻辑

1. 视频作为第一个 `-i`，音频依次作为后续 `-i`
2. FilterGraph 只处理音频流（视频流直接 `-map 0:v`）
3. 所有音频经 volume + adelay 后 amix
4. 输出：`-map 0:v -map "[amixed]" -c:v copy -c:a aac output`
5. 视频不重编码（stream copy），只编码混合后的音频

---

## 4. 音画对齐

```rust
// crates/core/src/ffmpeg/alignment.rs

#[derive(Debug, Clone, PartialEq)]
pub enum AlignmentStrategy {
    /// Durations match within tolerance
    Exact,
    /// Audio is shorter, pad with silence at the end
    PadSilence { gap_ms: i64 },
    /// Audio is slightly longer, trim the end
    TrimAudio { excess_ms: i64 },
    /// Audio is much longer, suggest regeneration with faster speed
    RegenerateNeeded { excess_ms: i64, suggested_speed: f64 },
}

/// Calculate alignment strategy for audio vs video duration
///
/// - tolerance_ms: acceptable duration difference (default 500ms)
/// - trim_threshold_ms: max excess before suggesting regeneration (default 3000ms)
pub fn calculate_alignment(
    video_duration_ms: i64,
    audio_duration_ms: i64,
    tolerance_ms: i64,
    trim_threshold_ms: i64,
) -> AlignmentStrategy;

/// Apply alignment: returns the processed audio path
pub fn apply_alignment(
    config: &FfmpegConfig,
    audio_path: &Path,
    video_duration_ms: i64,
    strategy: &AlignmentStrategy,
    output_dir: &Path,
) -> Result<PathBuf>;
```

`calculate_alignment` 逻辑：
```
diff = audio_duration - video_duration

if abs(diff) <= tolerance  → Exact
if diff < -tolerance       → PadSilence { gap_ms: abs(diff) }
if diff > tolerance && diff <= trim_threshold → TrimAudio { excess_ms: diff }
if diff > trim_threshold   → RegenerateNeeded { excess_ms: diff, suggested_speed: audio/video ratio }
```

---

## 5. IPC 命令

| command | 签名 | 说明 |
|---|---|---|
| `probe_audio_duration` | `(path: String) -> Result<i64, String>` | 音频时长 ms |
| `check_audio_alignment` | `(video_asset_id: String, audio_asset_id: String) -> Result<AlignmentInfo, String>` | 返回对齐策略 + 时长信息 |

```rust
#[derive(Serialize, Type, TS)]
#[serde(rename_all = "camelCase")]
pub struct AlignmentInfo {
    pub video_duration_ms: i64,
    pub audio_duration_ms: i64,
    pub strategy: String,        // "exact" | "pad_silence" | "trim_audio" | "regenerate_needed"
    pub diff_ms: i64,
    pub suggested_speed: Option<f64>,
}
```

---

## 6. 前端组件

本 spec 前端极少，主要是数据层：

```
src/components/audio/
└── AlignmentBadge.tsx           # 音画对齐状态徽章
```

**AlignmentBadge**：小型 badge 显示在 shot 详情中（配音旁边）：
- 绿色 ✓：Exact
- 黄色 ⚠：PadSilence / TrimAudio（自动处理）
- 红色 ✗：RegenerateNeeded（需人工干预）

---

## 7. 目录增量

```
crates/core/src/ffmpeg/
├── mod.rs               # 追加 pub mod audio; pub mod filter_graph; pub mod alignment;
├── filter_graph.rs      # new
├── audio.rs             # new
└── alignment.rs         # new

src-tauri/src/commands/
└── audio.rs             # 追加 probe/alignment 命令

src/components/audio/
└── AlignmentBadge.tsx   # new
```

---

## 8. 测试

| 层级 | 用例 | 说明 |
|------|------|------|
| Rust 单元 | `filter_graph_empty` | 空 graph → 空字符串 |
| Rust 单元 | `filter_graph_single_volume` | `[0:a]volume=0.8[a0]` |
| Rust 单元 | `filter_graph_volume_adelay_chain` | 两个 node 正确链接 |
| Rust 单元 | `filter_graph_amix_two_inputs` | `[a0][a1]amix=inputs=2:duration=longest[out]` |
| Rust 单元 | `filter_graph_complex_3_inputs` | 3 输入完整图 |
| Rust 单元 | `filter_graph_aconcat` | 串联多段音频 |
| Rust 单元 | `alignment_exact` | diff < tolerance → Exact |
| Rust 单元 | `alignment_pad_silence` | audio 短 2s → PadSilence |
| Rust 单元 | `alignment_trim_audio` | audio 长 1s → TrimAudio |
| Rust 单元 | `alignment_regenerate` | audio 长 5s → RegenerateNeeded |
| Rust 单元 | `alignment_suggested_speed` | 正确计算建议语速 |
| Rust 单元 | `mix_audio_empty_inputs_rejected` | 空列表报错 |
| Rust 单元 | `mix_audio_invalid_volume_rejected` | volume 超范围报错 |
| Rust 集成 | `mix_two_audio_files` | 两个音频混合后输出正确时长 |
| Rust 集成 | `pad_audio_extends_duration` | 补静音后时长匹配目标 |
| Rust 集成 | `trim_audio_shortens` | 裁剪后时长匹配 |
| Rust 集成 | `overlay_audio_on_video` | 输出有视频+音频流 |
| Rust 集成 | `probe_audio_duration_mp3` | 正确解析 mp3 时长 |
| Rust 集成 | `probe_audio_duration_wav` | 正确解析 wav 时长 |

注：集成测试需本地安装 ffmpeg，标记 `#[cfg(feature = "ffmpeg-tests")]` 或 `#[ignore]`。

---

## 9. 验收清单

- [ ] FilterGraph builder 生成正确的 -filter_complex 字符串
- [ ] `mix_audio` 合并 N 个音频文件，支持音量和延迟
- [ ] `overlay_audio_on_video` 将音频覆盖到视频（视频不重编码）
- [ ] `pad_audio_to_duration` 用静音填充到目标时长
- [ ] `trim_audio` 裁剪音频到指定时长
- [ ] `probe_audio_duration` 正确返回 mp3/wav/ogg 时长
- [ ] `calculate_alignment` 四种策略判断正确
- [ ] `apply_alignment` 执行 pad/trim 操作
- [ ] 所有操作通过 progress callback 报告进度
- [ ] 错误消息清晰（文件不存在、ffmpeg 执行失败等）
- [ ] 集成测试覆盖真实 ffmpeg 执行
- [ ] `cargo clippy` 无新增 warning
