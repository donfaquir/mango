# SPEC-33: 视频操作命令封装（裁剪 / 分割 / 拼接 / 缩略图）

> 对应 MS5 原任务 2（裁剪）、3（分割）、4（拼接）。在 spec-32 sidecar 基础上封装核心视频操作命令。

依赖：spec-32（FFmpeg sidecar + probe）

非目标：
- 进度解析与事件推送 — spec-34（本 spec 的命令函数预留 progress sender 参数，但不实现解析逻辑）
- 前端 UI — spec-35/36
- 字幕烧录 / 气泡叠加 — MS6

---

## 1. 裁剪（Trim）

```rust
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

pub async fn trim_video(
    config: &FfmpegConfig,
    input: TrimInput,
    progress_tx: Option<mpsc::Sender<FfmpegProgress>>,
) -> Result<PathBuf>
```

**Copy 模式**：`ffmpeg -ss {start} -to {end} -i {input} -c copy {output}`
- `-ss` 放在 `-i` 前面，利用 seek 加速
- 裁剪点对齐到最近关键帧，可能有 ±0.5s 偏差

**Reencode 模式**：`ffmpeg -i {input} -ss {start} -to {end} -c:v libx264 -preset fast -crf 18 -c:a aac {output}`
- `-ss` 放在 `-i` 后面，精确到帧
- 编码参数使用合理默认值（crf 18 = 高质量）

**参数校验**：
- `start_ms >= 0`
- `end_ms > start_ms`
- `end_ms <= duration_ms`（通过 probe 获取）
- 输出路径的父目录存在

---

## 2. 分割（Split）

```rust
pub struct SplitInput {
    pub input: PathBuf,
    pub split_points_ms: Vec<i64>,
    pub output_dir: PathBuf,
    pub mode: TrimMode,
}

pub async fn split_video(
    config: &FfmpegConfig,
    input: SplitInput,
    progress_tx: Option<mpsc::Sender<FfmpegProgress>>,
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

## 3. 拼接（Concat）

```rust
pub enum ConcatMode {
    Demuxer,        // -f concat -c copy，同参数快速拼接
    FilterComplex,  // filter_complex，异参数需重编码
}

pub struct ConcatInput {
    pub inputs: Vec<PathBuf>,
    pub output: PathBuf,
    pub mode: Option<ConcatMode>,  // None = 自动检测
}

pub async fn concat_videos(
    config: &FfmpegConfig,
    input: ConcatInput,
    progress_tx: Option<mpsc::Sender<FfmpegProgress>>,
) -> Result<PathBuf>
```

**Demuxer 模式**（同参数）：
1. 创建临时 concat list 文件：`file '/path/to/a.mp4'\nfile '/path/to/b.mp4'`
2. `ffmpeg -f concat -safe 0 -i {list} -c copy {output}`

**FilterComplex 模式**（异参数）：
1. 对所有输入 probe，取最大分辨率 + 最高帧率作为目标
2. `ffmpeg -i a.mp4 -i b.mp4 -filter_complex "[0:v]scale=W:H,setsar=1[v0];[1:v]scale=W:H,setsar=1[v1];[v0][0:a][v1][1:a]concat=n=2:v=1:a=1[outv][outa]" -map "[outv]" -map "[outa]" {output}`

**自动检测**（`mode=None`）：对所有输入 probe，如果分辨率、帧率、编解码器均一致则用 Demuxer，否则用 FilterComplex。

**校验**：
- 至少 2 个输入文件
- 所有输入文件存在且可读

---

## 4. 缩略图抽取

### 4.1 单帧缩略图

```rust
pub async fn extract_thumbnail(
    config: &FfmpegConfig,
    input: &Path,
    timestamp_ms: i64,
    output: &Path,
) -> Result<PathBuf>
```

`ffmpeg -ss {t} -i {input} -frames:v 1 -q:v 2 {output}`

输出格式由 output 扩展名决定（jpg/png/webp）。

### 4.2 缩略图条（时间轴用）

```rust
pub struct ThumbnailStripResult {
    pub thumbnails: Vec<PathBuf>,
    pub interval_ms: i64,
    pub count: u32,
}

pub async fn extract_thumbnail_strip(
    config: &FfmpegConfig,
    input: &Path,
    interval_ms: i64,
    thumb_width: u32,
    output_dir: &Path,
) -> Result<ThumbnailStripResult>
```

`ffmpeg -i {input} -vf "fps=1/{interval_s},scale={width}:-1" {output_dir}/%04d.jpg`

缩略图存放在项目 `thumbnails/strips/{asset_id}/` 下。前端拼接为时间轴背景。

**缓存策略**：按 `{asset_id}_{interval_ms}_{width}` 作为缓存 key，已存在则跳过生成。

---

## 5. 目录增量

```
crates/core/src/ffmpeg/
├── mod.rs          # + pub mod commands; pub mod thumbnail;
├── sidecar.rs      # (spec-32)
├── probe.rs        # (spec-32)
├── commands.rs     # trim_video, split_video, concat_videos
└── thumbnail.rs    # extract_thumbnail, extract_thumbnail_strip

src-tauri/src/commands/
└── ffmpeg.rs       # + trim_video, split_video, concat_videos,
                    #   extract_thumbnail, extract_thumbnail_strip
```

---

## 6. IPC 一览

| command | 签名 | 说明 |
|---|---|---|
| `trim_video` | `(input: String, start_ms: i64, end_ms: i64, output: String, mode: String) -> Result<String, String>` | 裁剪视频 |
| `split_video` | `(input: String, split_points_ms: Vec<i64>, output_dir: String, mode: String) -> Result<Vec<String>, String>` | 分割视频 |
| `concat_videos` | `(inputs: Vec<String>, output: String, mode: Option<String>) -> Result<String, String>` | 拼接视频 |
| `extract_thumbnail` | `(input: String, timestamp_ms: i64, output: String) -> Result<String, String>` | 单帧缩略图 |
| `extract_thumbnail_strip` | `(input: String, interval_ms: i64, thumb_width: u32, output_dir: String) -> Result<ThumbnailStripResult, String>` | 缩略图条 |

---

## 7. Model 变更

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
pub enum TrimMode {
    Copy,
    Reencode,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
pub enum ConcatMode {
    Demuxer,
    FilterComplex,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
pub struct ThumbnailStripResult {
    pub thumbnails: Vec<String>,  // relative paths
    pub interval_ms: i64,
    pub count: u32,
}
```

---

## 8. 测试

| 用例 | 说明 |
|---|---|
| `trim_copy_mode` | Copy 模式裁剪输出文件存在且可播放（需 fixture 视频） |
| `trim_reencode_mode` | Reencode 模式裁剪精度验证 |
| `trim_invalid_range` | `start >= end` 或 `end > duration` 返回错误 |
| `split_two_points` | 2 个分割点产出 3 个文件 |
| `split_empty_points` | 空分割点列表返回错误 |
| `concat_same_params` | 同参数拼接走 Demuxer 模式 |
| `concat_diff_params` | 异参数拼接走 FilterComplex 模式 |
| `concat_auto_detect` | 自动检测模式正确选择 |
| `concat_single_input` | 单个输入返回错误 |
| `thumbnail_single` | 指定时间点抽取缩略图存在 |
| `thumbnail_strip` | 缩略图条数量 ≈ duration / interval |
| `thumbnail_strip_cache` | 二次调用跳过生成 |

**注意**：需要测试用 fixture 视频文件。建议放置 2 个小视频（<1MB each）在 `crates/core/tests/fixtures/` 下，一个 5s mp4 (h264/aac)，一个 3s webm (vp9/opus)。测试标记 `#[ignore]` 供无 FFmpeg 环境跳过。

---

## 9. 验收清单

- [ ] `trim_video` Copy 模式输出可播放，时长约等于 `end - start`
- [ ] `trim_video` Reencode 模式裁剪精确到帧
- [ ] 非法时间参数（start >= end, end > duration）返回明确错误
- [ ] `split_video` 产出文件数 = 分割点数 + 1，各段可独立播放
- [ ] `concat_videos` 同参数片段快速拼接（Demuxer），输出连续
- [ ] `concat_videos` 异参数片段自动转码对齐（FilterComplex）
- [ ] `extract_thumbnail` 输出图片尺寸合理、内容对应指定时间点
- [ ] `extract_thumbnail_strip` 输出缩略图数量与间隔/时长一致
- [ ] 缩略图条二次调用使用缓存，不重复生成
