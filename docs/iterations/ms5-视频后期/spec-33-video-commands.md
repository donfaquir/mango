# SPEC-33: 视频操作命令封装（裁剪 / 分割 / 拼接 / 缩略图）

> 对应 MS5 原任务 2（裁剪）、3（分割）、4（拼接）。在 spec-32 sidecar 基础上封装核心视频操作命令。

依赖：spec-32（FFmpeg sidecar + probe）

非目标：
- 进度解析与事件推送 — spec-34（本 spec 的命令函数为纯功能实现，spec-34 再加进度回调）
- 缩略图条生成（`extract_thumbnail_strip`）— spec-35（涉及 asset_id 缓存和项目目录结构，属于时间轴业务层）
- 前端 UI — spec-35/36
- 字幕烧录 / 气泡叠加 — MS6
- 异参数视频拼接（FilterComplex 模式）— 后续按需添加

---

## 1. 设计决策

### 1.1 同步函数，不用 async

所有命令函数使用同步签名（`std::process::Command`），与 spec-32 的 `probe_video` / `check_ffmpeg` 保持一致。Tauri command 层已经是 async 的，由调用方决定是否用 `spawn_blocking` 包装。core crate 不引入 async 运行时依赖。

### 1.2 不预留 progress_tx 参数

spec-34 的 `FfmpegProgress` 类型在本 spec 实现时尚不存在。先写纯功能实现，spec-34 再扩展函数签名添加进度回调，保持每个 spec 自包含。

### 1.3 拼接只做 Demuxer 模式（MVP）

FilterComplex 模式（异参数转码拼接）涉及大量边界情况（音轨有无混合、像素格式对齐、采样率重采样、N 路 filter chain 拼接）。Mango 的使用场景是 AI 生成的视频片段——同一模型产出的视频参数大概率一致。MVP 阶段只实现 Demuxer（`-c copy`）模式，检测到参数不一致时返回明确错误提示用户，不尝试转码。

---

## 2. 裁剪（Trim）

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
pub enum TrimMode {
    Copy,      // -c copy，快速但关键帧对齐
    Reencode,  // 重编码，精确到帧
}

pub struct TrimInput {
    pub input: PathBuf,
    pub start_ms: i64,
    pub end_ms: i64,
    pub output: PathBuf,
    pub mode: TrimMode,
}

pub fn trim_video(
    config: &FfmpegConfig,
    input: TrimInput,
) -> Result<PathBuf>
```

**Copy 模式**：`ffmpeg -y -ss {start} -to {end} -i {input} -c copy {output}`
- `-ss` 放在 `-i` 前面，利用 seek 加速
- 裁剪点对齐到最近关键帧，可能有 ±0.5s 偏差

**Reencode 模式**：
- 先 probe 输入文件，检测是否有音轨
- 有音轨：`ffmpeg -y -i {input} -ss {start} -to {end} -c:v libx264 -preset fast -crf 18 -c:a aac {output}`
- 无音轨：`ffmpeg -y -i {input} -ss {start} -to {end} -c:v libx264 -preset fast -crf 18 -an {output}`
- `-ss` 放在 `-i` 后面，精确到帧

**编码器降级**：如果 `libx264` 不可用（可通过 `ffmpeg -encoders` 检测），回退到不指定编码器让 FFmpeg 自动选择输出格式的默认编码器。

**参数校验**：
- `start_ms >= 0`
- `end_ms > start_ms`
- `end_ms <= duration_ms`（通过 probe 获取）
- 输出路径的父目录存在

**时间格式化**：毫秒转为 FFmpeg 时间字符串 `HH:MM:SS.mmm`，封装为工具函数 `ms_to_ffmpeg_time(ms: i64) -> String`。

---

## 3. 分割（Split）

```rust
pub struct SplitInput {
    pub input: PathBuf,
    pub split_points_ms: Vec<i64>,
    pub output_dir: PathBuf,
    pub mode: TrimMode,
}

pub fn split_video(
    config: &FfmpegConfig,
    input: SplitInput,
) -> Result<Vec<PathBuf>>
```

**实现**：将 N 个分割点转换为 N+1 段 trim 调用。

例如 `split_points = [5000, 10000]` 在总时长 15000ms 的视频上产生：
- `{stem}_part1.mp4`: 0–5000ms
- `{stem}_part2.mp4`: 5000–10000ms
- `{stem}_part3.mp4`: 10000–15000ms

**校验**：
- 分割点严格递增
- 所有分割点在 `(0, duration_ms)` 范围内
- 至少 1 个分割点

---

## 4. 拼接（Concat）

```rust
pub struct ConcatInput {
    pub inputs: Vec<PathBuf>,
    pub output: PathBuf,
}

pub fn concat_videos(
    config: &FfmpegConfig,
    input: ConcatInput,
) -> Result<PathBuf>
```

**Demuxer 模式**（`-c copy`，快速无损拼接）：

1. 对所有输入 probe，检查分辨率、帧率、视频编解码器是否一致
2. 不一致时返回错误：`"cannot concat: input videos have different parameters (resolution/fps/codec). Please trim them to the same format first."`
3. 创建临时 concat list 文件：`file '/path/to/a.mp4'\nfile '/path/to/b.mp4'`
4. `ffmpeg -y -f concat -safe 0 -i {list} -c copy {output}`
5. 删除临时 list 文件

**校验**：
- 至少 2 个输入文件
- 所有输入文件存在且可读

**一致性判定规则**：
- 分辨率：`width` 和 `height` 完全相同
- 帧率：差异 < 0.01
- 视频编解码器：字符串相同
- 音轨：要么全有音轨，要么全无（混合则报错）

---

## 5. 单帧缩略图

```rust
pub fn extract_thumbnail(
    config: &FfmpegConfig,
    input: &Path,
    timestamp_ms: i64,
    output: &Path,
) -> Result<PathBuf>
```

`ffmpeg -y -ss {t} -i {input} -frames:v 1 -q:v 2 {output}`

输出格式由 output 扩展名决定（jpg/png/webp）。

**校验**：
- `timestamp_ms >= 0`
- 输入文件存在
- 输出路径的父目录存在

---

## 6. 工具函数

```rust
/// Convert milliseconds to FFmpeg time string: "HH:MM:SS.mmm"
pub fn ms_to_ffmpeg_time(ms: i64) -> String

/// Check if a specific encoder (e.g. "libx264") is available
pub fn has_encoder(config: &FfmpegConfig, encoder: &str) -> bool
```

`has_encoder` 执行 `ffmpeg -encoders` 并在输出中搜索编码器名。结果可在进程生命周期内缓存（`OnceLock<HashSet<String>>`）。

---

## 7. 目录增量

```
crates/core/src/ffmpeg/
├── mod.rs          # + pub mod commands;
├── sidecar.rs      # (spec-32)
├── probe.rs        # (spec-32)
└── commands.rs     # trim_video, split_video, concat_videos,
                    # extract_thumbnail, ms_to_ffmpeg_time, has_encoder

src-tauri/src/commands/
└── ffmpeg.rs       # + trim_video, split_video, concat_videos,
                    #   extract_thumbnail
```

---

## 8. IPC 一览

| command | 签名 | 说明 |
|---|---|---|
| `trim_video` | `(input: String, start_ms: i64, end_ms: i64, output: String, mode: TrimMode) -> Result<String, String>` | 裁剪视频，返回输出路径 |
| `split_video` | `(input: String, split_points_ms: Vec<i64>, output_dir: String, mode: TrimMode) -> Result<Vec<String>, String>` | 分割视频，返回各段路径 |
| `concat_videos` | `(inputs: Vec<String>, output: String) -> Result<String, String>` | 拼接视频（Demuxer 模式），返回输出路径 |
| `extract_thumbnail` | `(input: String, timestamp_ms: i64, output: String) -> Result<String, String>` | 单帧缩略图，返回输出路径 |

IPC 层的 `mode` 参数直接使用 `TrimMode` enum（tauri-specta 自动生成 TS 类型），不再用 `String`。

---

## 9. Model 变更

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export)]
pub enum TrimMode {
    Copy,
    Reencode,
}
```

`ConcatMode` 不再需要（MVP 只有 Demuxer 模式）。`ThumbnailStripResult` 移至 spec-35。

---

## 10. 测试

### 10.1 纯逻辑测试（不需要 FFmpeg）

| 用例 | 说明 |
|---|---|
| `ms_to_ffmpeg_time` | `0 → "00:00:00.000"`, `90061 → "00:01:30.061"`, `3661500 → "01:01:01.500"` |
| `trim_invalid_range` | `start >= end` 返回错误 |
| `trim_negative_start` | `start < 0` 返回错误 |
| `split_empty_points` | 空分割点列表返回错误 |
| `split_non_increasing` | `[5000, 3000]` 返回错误 |
| `concat_single_input` | 单个输入返回错误 |
| `concat_param_mismatch` | 分辨率不同时返回错误（mock probe 结果） |

### 10.2 集成测试（需要 FFmpeg，`#[ignore]`）

| 用例 | 说明 |
|---|---|
| `trim_copy_real` | Copy 模式裁剪 lavfi 测试视频，输出存在且 probe 时长正确 |
| `trim_reencode_real` | Reencode 模式裁剪，精度验证 |
| `split_two_points_real` | 2 个分割点产出 3 个文件，各段可 probe |
| `concat_same_params_real` | 两个同参数片段拼接，输出时长 ≈ 两段之和 |
| `thumbnail_real` | 抽取指定时间点缩略图，输出文件存在且是 JPEG |

测试视频使用 lavfi 测试源动态生成（与 `ffmpeg_smoke.rs` 相同方式），不提交 fixture 文件到仓库。

---

## 11. 验收清单

- [ ] `trim_video` Copy 模式输出可播放，时长约等于 `end - start`
- [ ] `trim_video` Reencode 模式裁剪精确到帧
- [ ] Reencode 对无音轨视频不报错（自动 `-an`）
- [ ] 非法时间参数（start >= end, end > duration）返回明确错误
- [ ] `split_video` 产出文件数 = 分割点数 + 1，各段可独立播放
- [ ] `concat_videos` 同参数片段快速拼接，输出连续
- [ ] `concat_videos` 异参数片段返回明确错误（不静默转码）
- [ ] `extract_thumbnail` 输出图片存在、尺寸合理
- [ ] `ms_to_ffmpeg_time` 覆盖边界值（0, 大数, 带毫秒）
