# SPEC-34: FFmpeg 进度解析与事件推送

> 对应 MS5 原任务 5（进度解析）。为所有耗时 FFmpeg 操作提供实时进度反馈。

依赖：spec-32（sidecar）、spec-33（命令函数 + `run_ffmpeg` helper）

非目标：
- 时间轴 UI — spec-35
- 导出 UI — spec-36
- CLI 进度输出 — spec-36（CLI `mango export clip` 实现时一并做）
- 进度条的前端样式/动画细节 — 在 spec-35/36 各自实现

---

## 1. 设计决策

### 1.1 扩展 spec-33 的 `run_ffmpeg`，而非预留参数

spec-33 的命令函数是纯同步的，`run_ffmpeg` 使用 `Command::output()` 阻塞等待进程退出。要实时解析进度，需要改为 `spawn()` + 流式读 stderr。

做法：**保留原有 `run_ffmpeg` 不变**（供不需要进度的场景如 `extract_thumbnail`、`has_encoder`），新增 `run_ffmpeg_with_progress` 变体。然后在 `trim_video`、`split_video`、`concat_videos` 中增加一个可选的 progress callback 参数。

### 1.2 不需要 `operation_id`（MVP）

Mango 的使用场景是用户手动触发裁剪/拼接，不会有多个 FFmpeg 操作同时跑。导出流程（trim → concat）也是顺序执行。MVP 去掉 `operation_id`，前端 hook 直接监听最新进度事件。如果将来需要并发 FFmpeg 操作再加 ID 区分。

### 1.3 CLI 进度推迟到 spec-36

CLI `mango export clip` 在 spec-36 实现，CLI 的进度输出（`\r` 覆写 stderr）放到那里一起做。spec-34 只做 core 层解析 + Tauri 事件推送 + 前端 hook。

---

## 2. FFmpeg stderr 解析

FFmpeg 在处理过程中向 stderr 输出类似：

```
frame=  120 fps= 30 q=28.0 size=    1024kB time=00:00:04.00 bitrate= 2097.2kbits/s speed=2.00x
```

关键字段：`time=HH:MM:SS.ff`。

```rust
pub struct FfmpegProgress {
    pub progress_pct: f64,      // 0.0 – 100.0
    pub current_time_ms: i64,
    pub total_duration_ms: i64,
    pub speed: Option<f64>,     // e.g. 2.0
}
```

**解析逻辑**（`FfmpegProgressParser`）：

```rust
pub struct FfmpegProgressParser {
    total_duration_ms: i64,
}

impl FfmpegProgressParser {
    pub fn new(total_duration_ms: i64) -> Self { ... }

    pub fn parse_line(&self, line: &str) -> Option<FfmpegProgress> {
        // 1. find "time=HH:MM:SS.ff" substring, parse to ms
        // 2. progress_pct = current_time_ms / total_duration_ms * 100.0
        // 3. find "speed=N.NNx" substring if present
        // return None if no time= found
    }
}
```

容错：
- 无法解析的行返回 `None`，不中断处理
- `total_duration_ms == 0` 时 `progress_pct` 固定为 0（避免除零）
- 时间解析复用 spec-33 的 `ms_to_ffmpeg_time` 的逆操作

---

## 3. `run_ffmpeg_with_progress`

新增函数，与现有 `run_ffmpeg` 并存：

```rust
pub fn run_ffmpeg_with_progress(
    cmd: &mut Command,
    total_duration_ms: i64,
    on_progress: impl FnMut(FfmpegProgress),
) -> Result<()>
```

**实现**：

1. `cmd.stderr(Stdio::piped()).spawn()` 启动子进程
2. `BufReader::new(child.stderr)` 逐行读取
3. 每行调 `parser.parse_line()`，有结果则调 `on_progress(progress)`
4. 进程退出后检查 exit code
5. 失败时收集剩余 stderr 返回错误

回调使用 `FnMut` 闭包而非 channel——调用方自行决定是发 channel、写 stderr 还是其他。更灵活，不绑定特定通信方式。

**修改 spec-33 命令函数**：

给 `trim_video`、`split_video`、`concat_videos` 增加可选 callback：

```rust
pub fn trim_video(
    config: &FfmpegConfig,
    input: &Path,
    start_ms: i64,
    end_ms: i64,
    output: &Path,
    mode: &TrimMode,
    on_progress: Option<&mut dyn FnMut(FfmpegProgress)>,
) -> Result<PathBuf>
```

`on_progress` 为 `None` 时走原有的 `run_ffmpeg`（`Command::output()`），保持现有行为不变。为 `Some` 时走 `run_ffmpeg_with_progress`。

---

## 4. Tauri 事件桥接

复用现有 `events.rs` 的模式（`tauri-specta` Event derive）：

```rust
#[derive(Clone, Debug, Serialize, Deserialize, Type, Event)]
pub struct FfmpegProgressTick {
    pub progress_pct: f64,
    #[specta(type = specta_typescript::Number)]
    pub current_time_ms: i64,
    #[specta(type = specta_typescript::Number)]
    pub total_duration_ms: i64,
    pub speed: Option<f64>,
}
```

**Tauri command 层集成**：

```rust
#[tauri::command]
pub async fn trim_video(app: AppHandle, input: String, ...) -> Result<String, IpcError> {
    let app_clone = app.clone();
    tokio::task::spawn_blocking(move || {
        let mut on_progress = |p: FfmpegProgress| {
            let _ = FfmpegProgressTick { ... }.emit(&app_clone);
        };
        ffmpeg::trim_video(&config, ..., Some(&mut on_progress))
    }).await.map_err(...)
}
```

命令函数变为 blocking task（因为 FFmpeg 进程 I/O 是阻塞的），通过 `emit` 实时推送事件到前端。

---

## 5. 前端 hook

```typescript
// src/hooks/useFFmpegProgress.ts

import { useEffect, useState } from "react";
import { events } from "@/lib/bindings/commands";

interface FFmpegProgress {
    progressPct: number;
    currentTimeMs: number;
    totalDurationMs: number;
    speed: number | null;
}

export function useFFmpegProgress(): FFmpegProgress | null {
    const [progress, setProgress] = useState<FFmpegProgress | null>(null);

    useEffect(() => {
        const unlisten = events.ffmpegProgressTick.listen((e) => {
            setProgress(e.payload);
        });
        return () => { unlisten.then((fn) => fn()); };
    }, []);

    return progress;
}
```

无 `operationId` 过滤——直接返回最新一条进度。调用方在 FFmpeg 操作开始时 reset 为 null，操作结束时忽略后续事件。

---

## 6. 目录增量

```
crates/core/src/ffmpeg/
├── mod.rs          # + pub mod progress;
├── commands.rs     # 修改：trim/split/concat 增加 on_progress 参数
└── progress.rs     # FfmpegProgress, FfmpegProgressParser,
                    # run_ffmpeg_with_progress

src-tauri/src/
├── events.rs       # + FfmpegProgressTick
└── commands/
    └── ffmpeg.rs   # 修改：注入 on_progress callback + emit 事件

src/hooks/
└── useFFmpegProgress.ts    # 新增
```

---

## 7. IPC 一览

无新增 command。进度通过 tauri-specta Event 推送：

| 事件 | payload | 方向 |
|---|---|---|
| `FfmpegProgressTick` | `{ progress_pct, current_time_ms, total_duration_ms, speed }` | backend → frontend |

需在 `lib.rs` 的 `collect_events!` 中注册。

---

## 8. Model 变更

Core 层（`progress.rs`）：

```rust
#[derive(Debug, Clone)]
pub struct FfmpegProgress {
    pub progress_pct: f64,
    pub current_time_ms: i64,
    pub total_duration_ms: i64,
    pub speed: Option<f64>,
}
```

Tauri 事件（`events.rs`）：

```rust
#[derive(Clone, Debug, Serialize, Deserialize, Type, Event)]
pub struct FfmpegProgressTick {
    pub progress_pct: f64,
    #[specta(type = specta_typescript::Number)]
    pub current_time_ms: i64,
    #[specta(type = specta_typescript::Number)]
    pub total_duration_ms: i64,
    pub speed: Option<f64>,
}
```

---

## 9. 测试

### 9.1 纯逻辑测试（不需要 FFmpeg）

| 用例 | 说明 |
|---|---|
| `parse_valid_line` | `time=00:00:04.50` + total 10s → `progress_pct ≈ 45.0` |
| `parse_with_speed` | `speed=2.00x` → `speed = Some(2.0)` |
| `parse_no_time` | 无 `time=` 的行返回 `None` |
| `parse_zero_duration` | total_duration_ms=0 → pct=0，不 panic |
| `parse_various_formats` | `time=01:02:03.45`、`time=00:00:00.00` 等边界值 |

### 9.2 集成测试（需要 FFmpeg，`#[ignore]`）

| 用例 | 说明 |
|---|---|
| `trim_with_progress` | 裁剪 5s 视频，on_progress 收到 ≥3 次调用，pct 最终 ≈100 |
| `progress_none_still_works` | on_progress=None 时 trim 正常完成（回归） |

---

## 10. 验收清单

- [ ] `FfmpegProgressParser::parse_line` 正确解析 `time=` 和 `speed=`
- [ ] `run_ffmpeg_with_progress` 流式读取 stderr 并调用 callback
- [ ] 裁剪 5s 视频时 on_progress 被调用多次，pct 单调递增
- [ ] `on_progress=None` 时命令函数行为与 spec-33 一致（回归）
- [ ] `FfmpegProgressTick` 事件在 Tauri 前端可接收
- [ ] `useFFmpegProgress` hook 返回最新进度
- [ ] 进度更新频率合理（约每秒 1-2 次）
