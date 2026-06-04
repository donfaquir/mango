# MS5 新增依赖一览

本文档汇总 MS5 五个 spec（32–36）中提议引入的依赖与 migration，便于一次性评审。**版本号在实施 PR 时按项目规则核对最新 stable 后再 pin**。

---

## 1. npm 依赖（`package.json`）

| 包 | 用途 | 引入 spec | 备注 |
|---|---|---|---|
| 无新增（预期） | 时间轴自研 div-based；拖拽复用 `@dnd-kit` | spec-35, spec-36 | `@dnd-kit/core` + `@dnd-kit/sortable` 已在项目中 |

### 1.1 shadcn/ui 组件（按需 `pnpm dlx shadcn@latest add ...`）

| 组件 | 用途 | 引入 spec |
|---|---|---|
| `slider` | 时间轴缩放控制 | spec-35 |
| `sheet` | 导出设置面板 | spec-36 |

实施前用 `ls src/components/ui` 核对是否已有，避免重复 add。

---

## 2. Rust 依赖（workspace）

| crate | 用途 | 引入 spec | 备注 |
|---|---|---|---|
| `ffmpeg-sidecar` | FFmpeg 进程管理与输出解析 | spec-32 | 技术选型文档指定 2.5+；实施时 WebSearch 确认最新版本。**不使用其 `auto_download` 功能**——FFmpeg 二进制通过 Tauri `externalBin` 嵌入安装包，`FfmpegCommand::new_with_path()` 接受显式路径 |

添加位置：workspace `Cargo.toml` + `crates/core/Cargo.toml`。

### 2.1 FFmpeg 静态二进制（非 Rust 依赖）

FFmpeg + ffprobe 静态二进制嵌入安装包。各平台来源见 [spec-32 §1.1](spec-32-ffmpeg-sidecar.md#11-各平台二进制来源)。

| 平台 | 文件 | 放置位置 |
|---|---|---|
| macOS arm64 | `ffmpeg-aarch64-apple-darwin`, `ffprobe-aarch64-apple-darwin` | `src-tauri/binaries/` |
| macOS x64 | `ffmpeg-x86_64-apple-darwin`, `ffprobe-x86_64-apple-darwin` | `src-tauri/binaries/` |
| Windows x64 | `ffmpeg-x86_64-pc-windows-msvc.exe`, `ffprobe-x86_64-pc-windows-msvc.exe` | `src-tauri/binaries/` |
| Linux x64 | `ffmpeg-x86_64-unknown-linux-gnu`, `ffprobe-x86_64-unknown-linux-gnu` | `src-tauri/binaries/` |

开发环境走系统 PATH，`src-tauri/binaries/` 下无需放置 FFmpeg（`.gitignore` 忽略）。

---

## 3. Tauri 插件

无新增 Tauri 插件。`tauri-plugin-shell` 已有但 FFmpeg 进程管理不走它（`ffmpeg-sidecar` 直接 spawn 子进程）。

---

## 4. 数据库 migration

| migration | 内容 | 引入 spec |
|---|---|---|
| `013_video_clip.sql` | `video_clip` 表：`id TEXT PK, project_id TEXT NOT NULL REFERENCES project(id), episode_id TEXT REFERENCES episode(id), source_asset_id TEXT NOT NULL REFERENCES asset(id), label TEXT, trim_start_ms INTEGER, trim_end_ms INTEGER, order_index INTEGER NOT NULL, created_at TEXT NOT NULL DEFAULT (datetime('now'))` + 索引 `idx_video_clip_episode` on `(episode_id, order_index)` | spec-36 |

视频探测元数据（时长、分辨率、编码等）存入 `asset.metadata_json`（已有列），无需 schema 变更。

---

## 5. Tauri command 增量（一览）

| command | 引入 spec | 用途 |
|---|---|---|
| `check_ffmpeg` | spec-32 | 返回 FFmpeg 可用性、版本、路径 |
| `probe_video` | spec-32 | 返回视频元数据（时长、分辨率、编解码器、帧率） |
| `trim_video` | spec-33 | 按起止时间裁剪视频 |
| `split_video` | spec-33 | 按多个时间点分割视频 |
| `concat_videos` | spec-33 | 将多个视频片段按顺序拼接 |
| `extract_thumbnail` | spec-33 | 抽取指定时间点的单帧缩略图 |
| `extract_thumbnail_strip` | spec-33 | 按间隔抽取缩略图序列（时间轴用） |
| `list_video_clips` | spec-36 | 按 episode 列出片段列表 |
| `create_video_clip` | spec-36 | 创建片段记录 |
| `update_video_clip` | spec-36 | 更新裁剪范围/标签 |
| `delete_video_clip` | spec-36 | 删除片段 |
| `reorder_video_clips` | spec-36 | 批量更新 order_index |
| `export_concat` | spec-36 | 按片段列表裁剪+拼接并导出 |

---

## 6. CLI 子命令增量

| 命令 | 引入 spec |
|---|---|
| `mango export clip --project <id> --episode <id> --output <path>` | spec-36 |

可选参数：`--resolution 1080p --format mp4`。进度输出到 stderr。

---

## 7. 建议 PR 实施顺序

```
spec-32 (FFmpeg sidecar + probe)              ~6–8h
    └─▸ spec-33 (video commands)              ~10–12h
            └─▸ spec-34 (progress + events)   ~4–6h
                    └─▸ spec-35 (timeline UI)  ~12–16h
                            └─▸ spec-36 (clip assembly + export)  ~6–8h
```

- **spec-33 依赖 spec-32**：所有 FFmpeg 命令需要 sidecar 层的 `FfmpegCommand` 构建器。
- **spec-34 依赖 spec-32+33**：进度解析需要 sidecar 的 stderr 流（spec-32），命令函数接受 progress sender（spec-33）。
- **spec-35 依赖 spec-33+34**：缩略图条需要 `extract_thumbnail_strip`（spec-33），操作触发需要进度 hook（spec-34）。
- **spec-36 依赖 spec-33+34+35**：导出执行裁剪+拼接（spec-33），显示进度（spec-34），时间轴提供裁剪参数（spec-35）。
- **并行机会**：spec-35 的纯 UI 组件（手柄拖拽、刻度尺、播放头同步）可与 spec-34 并行开发，集成测试时需 spec-34。

---

## 8. 版本验证清单（实施 PR 前勾选）

- [ ] `pnpm outdated` — 无 MS5 强制升级的 npm 包时标 N/A
- [ ] `cargo outdated` — 检查 `ffmpeg-sidecar` 版本是否为最新 stable
- [ ] `ffmpeg -version` 在开发环境可执行（PATH 中可找到）
- [ ] migration 013 在空库 + 已有 012 的库上各跑一遍 `startup::initialize`
- [ ] `tauri.conf.json` 中 `externalBin` 数组更新后 `pnpm tauri dev` 正常启动
