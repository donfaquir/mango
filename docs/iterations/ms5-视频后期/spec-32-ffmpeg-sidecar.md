# SPEC-32: FFmpeg Sidecar 集成与视频探测

> 对应 MS5 原任务 1（FFmpeg sidecar 集成）。建立 FFmpeg 进程管理基础设施，为后续所有视频操作提供运行环境。

依赖：无（MS5 首个 spec）

非目标：
- 视频裁剪/分割/拼接命令 — spec-33
- 进度解析 — spec-34
- 任何前端 UI — spec-35/36

---

## 1. 打包策略（决议：嵌入安装包）

FFmpeg 静态二进制通过 Tauri `externalBin` 嵌入安装包，开箱即用、无需联网。不使用 `ffmpeg-sidecar` 的 `auto_download()`。

### 1.1 各平台二进制来源

| 目标平台 | Tauri target triple | FFmpeg 文件名 | 下载源 |
|---|---|---|---|
| macOS Apple Silicon | `aarch64-apple-darwin` | `ffmpeg-aarch64-apple-darwin` | [evermeet.cx](https://evermeet.cx/ffmpeg/) |
| macOS Intel | `x86_64-apple-darwin` | `ffmpeg-x86_64-apple-darwin` | [evermeet.cx](https://evermeet.cx/ffmpeg/) |
| Windows x64 | `x86_64-pc-windows-msvc` | `ffmpeg-x86_64-pc-windows-msvc.exe` | [gyan.dev](https://www.gyan.dev/ffmpeg/builds/) essentials |
| Linux x64 | `x86_64-unknown-linux-gnu` | `ffmpeg-x86_64-unknown-linux-gnu` | [johnvansickle.com](https://johnvansickle.com/ffmpeg/) static |

`ffprobe` 同理，每个平台各一份，放在同目录下。

**FFmpeg 版本**：最新 7.x stable release。使用 essentials/static 版本（含 h264/aac/vp9 等核心编解码器）。

**体积影响**：FFmpeg + ffprobe 约 80-120MB/平台，安装包总体积约 110-150MB。

### 1.2 二进制发现优先级

| 优先级 | 环境 | 查找方式 |
|---|---|---|
| 1 | 生产 (`tauri build`) | Tauri sidecar 解包路径（`app.path().resource_dir()`） |
| 2 | CLI (`mango-cli`) | `MANGO_FFMPEG_PATH` 环境变量 |
| 3 | 开发 / 回退 | 系统 PATH（`brew install ffmpeg` / `apt install ffmpeg`） |

`ffmpeg-sidecar` crate 的 `FfmpegCommand::new_with_path()` 接受显式路径，跳过其默认查找逻辑。生产环境由 Tauri 层解析 sidecar 路径后注入。

### 1.3 CI 打包脚本（概要）

```bash
# scripts/download-ffmpeg.sh
# 在 CI 构建前执行，按 $TAURI_TARGET_TRIPLE 下载对应平台的 FFmpeg 静态二进制
# 放到 src-tauri/binaries/ffmpeg-${TAURI_TARGET_TRIPLE} 和 ffprobe-${TAURI_TARGET_TRIPLE}
```

CI 脚本的完整实现不在本 spec 范围内（仅做目录和配置准备），可在首次 `tauri build` 时补充。

### 1.4 架构约束

core crate 不依赖 Tauri API。`FfmpegConfig` 在 core 中定义，Tauri 层在 app setup 时构造并注入 sidecar 解包路径（通过函数参数），CLI 层使用 `FfmpegConfig::from_env()`。

---

## 2. `ffmpeg-sidecar` 集成

添加 `ffmpeg-sidecar` 到 workspace 和 core crate 依赖。

core 新增 `ffmpeg` 模块：

```rust
// crates/core/src/ffmpeg/sidecar.rs

pub struct FfmpegConfig {
    pub ffmpeg_path: Option<PathBuf>,
    pub ffprobe_path: Option<PathBuf>,
}

impl FfmpegConfig {
    pub fn from_env() -> Self { /* MANGO_FFMPEG_PATH → PATH fallback */ }
    pub fn with_paths(ffmpeg: PathBuf, ffprobe: PathBuf) -> Self { /* explicit */ }
}
```

Tauri 层在 app setup 时构造 `FfmpegConfig`，传入 sidecar 解包路径；CLI 层使用 `FfmpegConfig::from_env()`。

---

## 3. 健康检查

```rust
pub struct FfmpegStatus {
    pub available: bool,
    pub version: Option<String>,    // e.g. "7.1.1"
    pub path: Option<PathBuf>,
    pub codecs: Vec<String>,        // 支持的编解码器名称列表
}

pub async fn check_ffmpeg(config: &FfmpegConfig) -> FfmpegStatus
```

执行 `ffmpeg -version`，解析第一行提取版本号。失败时返回 `available=false`，附 error message。

前端可在设置页或视频编辑器入口展示 FFmpeg 状态指示。

---

## 4. 视频探测（Probe）

```rust
pub struct VideoMetadata {
    pub duration_ms: i64,
    pub width: u32,
    pub height: u32,
    pub video_codec: String,        // e.g. "h264"
    pub audio_codec: Option<String>,// e.g. "aac"
    pub fps: f64,
    pub bitrate_kbps: Option<u32>,
    pub file_size_bytes: u64,
}

pub async fn probe_video(config: &FfmpegConfig, path: &Path) -> Result<VideoMetadata>
```

执行：`ffprobe -v quiet -print_format json -show_streams -show_format <path>`

解析 JSON 输出：
- `format.duration` → `duration_ms`（秒 → 毫秒）
- `streams[video].width/height/codec_name/r_frame_rate`
- `streams[audio].codec_name`
- `format.bit_rate` → `bitrate_kbps`
- `format.size` → `file_size_bytes`

探测结果可写入 `asset.metadata_json`（已有列），供前端展示视频信息。

---

## 5. Tauri `externalBin` 配置

`tauri.conf.json` 的 `bundle.externalBin` 数组追加：

```json
"externalBin": [
    "binaries/mango-cli",
    "binaries/ffmpeg",
    "binaries/ffprobe"
]
```

**注意**：FFmpeg 静态二进制约 80-120MB，会显著增大安装包体积。Tauri `externalBin` 期望二进制按平台后缀命名（如 `ffmpeg-aarch64-apple-darwin`）。具体打包流程需在 CI 中配置下载脚本——本 spec 只做配置，CI 脚本视为后续任务。

开发阶段 `binaries/` 目录下无需放置 FFmpeg 二进制（走 PATH 查找）。

---

## 6. 目录增量

```
crates/core/src/
└── ffmpeg/
    ├── mod.rs          # pub mod sidecar; pub mod probe; re-export
    ├── sidecar.rs      # FfmpegConfig, check_ffmpeg, FfmpegStatus
    └── probe.rs        # probe_video, VideoMetadata

src-tauri/src/
└── commands/
    └── ffmpeg.rs       # check_ffmpeg, probe_video Tauri commands
```

`crates/core/src/lib.rs` 新增 `pub mod ffmpeg;`

`src-tauri/src/commands/mod.rs` 新增 `pub mod ffmpeg;` 并注册 commands。

---

## 7. IPC 一览

| command | 签名 | 说明 |
|---|---|---|
| `check_ffmpeg` | `() -> Result<FfmpegStatus, String>` | 检查 FFmpeg 可用性和版本 |
| `probe_video` | `(path: String) -> Result<VideoMetadata, String>` | 探测视频文件元数据 |

---

## 8. Model 变更

```rust
#[derive(Debug, Serialize, Deserialize, Type, TS)]
pub struct FfmpegStatus {
    pub available: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub codecs: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Type, TS)]
pub struct VideoMetadata {
    pub duration_ms: i64,
    pub width: u32,
    pub height: u32,
    pub video_codec: String,
    pub audio_codec: Option<String>,
    pub fps: f64,
    pub bitrate_kbps: Option<u32>,
    pub file_size_bytes: u64,
}
```

---

## 9. 测试

| 用例 | 说明 |
|---|---|
| `version_parsing` | 解析 `ffmpeg version 7.1.1 Copyright ...` 提取 `"7.1.1"` |
| `version_parsing_n_series` | 解析 `ffmpeg version N-xxxxx` 格式 |
| `probe_json_parsing` | 从 ffprobe JSON 输出正确提取各字段 |
| `probe_no_audio` | 纯视频文件（无音轨）`audio_codec` 为 None |
| `missing_binary` | FFmpeg 不在 PATH 时返回 `available=false` |
| `env_var_override` | `MANGO_FFMPEG_PATH` 优先于 PATH |

---

## 10. 验收清单

- [ ] 开发环境下 `check_ffmpeg` 返回 `available=true` + 正确版本号
- [ ] `probe_video` 对 mp4/webm 文件返回正确的时长、分辨率、编解码器
- [ ] FFmpeg 不可用时 `check_ffmpeg` 返回 `available=false`，不 panic
- [ ] `MANGO_FFMPEG_PATH` 环境变量被正确使用
- [ ] core crate 无 Tauri API 依赖（编译 `cargo build -p mango-core` 不报错）
- [ ] `cargo test -p mango-core` FFmpeg 相关测试通过
