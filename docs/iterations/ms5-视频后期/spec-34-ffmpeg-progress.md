# SPEC-34: FFmpeg 进度解析与事件推送

> 对应 MS5 原任务 5（进度解析）。为所有耗时 FFmpeg 操作提供实时进度反馈。

依赖：spec-32（sidecar 的 stderr 流）、spec-33（命令函数预留的 `progress_tx` 参数）

非目标：
- 时间轴 UI — spec-35
- 导出 UI — spec-36
- 进度条的前端样式/动画细节 — 在 spec-35/36 各自实现

---

## 1. FFmpeg stderr 解析

FFmpeg 在处理过程中向 stderr 输出类似：

```
frame=  120 fps= 30 q=28.0 size=    1024kB time=00:00:04.00 bitrate= 2097.2kbits/s speed=2.00x
```

关键字段：`time=HH:MM:SS.ff`。

```rust
pub struct FfmpegProgress {
    pub operation_id: String,
    pub progress_pct: f64,      // 0.0 – 100.0
    pub current_time_ms: i64,   // 已处理到的时间点
    pub total_duration_ms: i64, // 源视频总时长
    pub speed: Option<f64>,     // 处理速度倍率 (e.g. 2.0x)
}
```

**解析逻辑**（`FfmpegProgressParser`）：

```rust
pub struct FfmpegProgressParser {
    total_duration_ms: i64,
    operation_id: String,
}

impl FfmpegProgressParser {
    pub fn parse_line(&self, line: &str) -> Option<FfmpegProgress> {
        // regex: time=(\d{2}):(\d{2}):(\d{2})\.(\d{2})
        // -> current_time_ms
        // progress_pct = current_time_ms / total_duration_ms * 100.0
        // also extract speed=(\d+\.\d+)x if present
    }
}
```

容错：无法解析的行返回 `None`，不中断处理。`total_duration_ms == 0` 时 `progress_pct` 固定为 0（避免除零）。

---

## 2. Core 层 progress channel

spec-33 中各命令函数已预留 `progress_tx: Option<mpsc::Sender<FfmpegProgress>>` 参数。

**集成方式**：

1. 命令函数启动 FFmpeg 子进程
2. 创建 `FfmpegProgressParser`（传入 total_duration_ms，来自 probe）
3. 逐行读取 stderr（`BufReader::lines()`）
4. 每行尝试 `parser.parse_line()`，成功则 `progress_tx.send()`
5. 进程退出后发送一个 `progress_pct = 100.0` 的终结消息

`progress_tx` 为 `None` 时跳过解析，直接等待进程退出——保持无 UI 场景的简洁性。

---

## 3. Tauri 事件桥接

```rust
// src-tauri/src/events.rs (or inline in ffmpeg commands)

#[derive(Clone, Serialize, Type, TS)]
pub struct FfmpegProgressTick {
    pub operation_id: String,
    pub progress_pct: f64,
    pub current_time_ms: i64,
    pub total_duration_ms: i64,
    pub speed: Option<f64>,
}
```

Tauri command 层在调用 core 命令时：
1. 创建 `mpsc::channel(32)`
2. 将 `tx` 传入 core 命令函数
3. Spawn 一个 tokio task drain `rx`，通过 `app_handle.emit("ffmpeg-progress-tick", tick)` 推送到前端
4. 命令完成后 `tx` drop，drain task 自动退出

事件名：`ffmpeg-progress-tick`（与任务引擎的 `task-progress-tick` 保持命名风格一致）。

---

## 4. CLI 进度输出

CLI 场景不走 Tauri 事件，直接在 stderr 输出：

```
Trimming video... 45% (4.5s / 10.0s) speed: 2.0x
```

使用 `\r` 覆写同一行，避免屏幕滚动。不引入额外 crate（如 `indicatif`）——简单 `eprint!("\r...")` 足够。

CLI 的 drain 逻辑：创建 `mpsc::channel`，spawn blocking task 读 `rx` 并 `eprint!`。

---

## 5. 前端 hook

```typescript
// src/hooks/useFFmpegProgress.ts

interface FFmpegProgress {
    operationId: string;
    progressPct: number;
    currentTimeMs: number;
    totalDurationMs: number;
    speed: number | null;
}

function useFFmpegProgress(operationId: string): FFmpegProgress | null
```

实现：
- `useEffect` 中调用 `listen("ffmpeg-progress-tick", callback)`
- callback 内过滤 `event.payload.operation_id === operationId`
- 更新 state
- cleanup 时 `unlisten()`

---

## 6. 目录增量

```
crates/core/src/ffmpeg/
├── mod.rs          # + pub mod progress;
└── progress.rs     # FfmpegProgress, FfmpegProgressParser

src-tauri/src/
└── events.rs       # + FfmpegProgressTick (或 inline)

src/hooks/
└── useFFmpegProgress.ts    # 新增
```

---

## 7. IPC 一览

无新增 command。进度通过 Tauri event（push-only）推送：

| 事件 | payload | 方向 |
|---|---|---|
| `ffmpeg-progress-tick` | `FfmpegProgressTick` | backend → frontend |

---

## 8. Model 变更

Core 层：

```rust
#[derive(Debug, Clone)]
pub struct FfmpegProgress {
    pub operation_id: String,
    pub progress_pct: f64,
    pub current_time_ms: i64,
    pub total_duration_ms: i64,
    pub speed: Option<f64>,
}
```

Tauri 事件 payload（需 Serialize + Type + TS）：

```rust
#[derive(Debug, Clone, Serialize, Type, TS)]
pub struct FfmpegProgressTick {
    pub operation_id: String,
    pub progress_pct: f64,
    pub current_time_ms: i64,
    pub total_duration_ms: i64,
    pub speed: Option<f64>,
}
```

---

## 9. 测试

| 用例 | 说明 |
|---|---|
| `parse_valid_line` | `time=00:00:04.50` + total 10s → `progress_pct ≈ 45.0` |
| `parse_speed` | `speed=2.00x` → `speed = Some(2.0)` |
| `parse_no_time` | 无 `time=` 的行返回 `None` |
| `parse_zero_duration` | total_duration_ms=0 不 panic，返回 pct=0 |
| `channel_drain` | 发 5 个 progress → rx 收到 5 个，顺序正确 |
| `channel_none_skip` | progress_tx=None 时命令函数仍正常执行 |

---

## 10. 验收清单

- [ ] 裁剪 10s 视频时进度从 0% 平滑增长到 100%
- [ ] 进度更新频率合理（约每秒 1-2 次）
- [ ] 前端 `useFFmpegProgress` 正确接收进度事件
- [ ] 多个并发操作的 `operation_id` 不互相干扰
- [ ] CLI 模式 stderr 显示进度百分比和速度
- [ ] FFmpeg 命令 `progress_tx=None` 时不报错、不解析 stderr
