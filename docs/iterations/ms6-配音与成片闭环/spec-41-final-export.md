# SPEC-41: 最终合成与导出

> 对应 MS6 原任务 8+9+10（最终合成 + 导出设置 UI + CLI 导出命令）。扩展现有 export pipeline，将视频 + 配音 + 音效 + BGM 合成为最终成片。

依赖：spec-38（配音 asset）、spec-39（shot_audio 绑定）、spec-40（音频 FFmpeg 操作）

非目标：
- 字幕 ASS 烧录 — 渲染复杂度高，移至 MS7
- 云端渲染 / 分布式导出
- 同时输出多种格式
- 单 shot 带音频预览（需实时混合，P2）
- 多集批量导出

---

## 1. 设计决策

### 1.1 Export v2 扩展而非替代

现有 `export.rs` 中的 `export_resolved_clips` 保留为"纯视频导出"（无音频）。新增 `export_final` 作为"带音频的完整导出"。两者共享 `resolve_clips` 基础逻辑。

```
export_resolved_clips (v1)  → 纯视频拼接，保留
export_final (v2)           → 视频 + 音频合成，新增
```

### 1.2 两步合成策略

1. **Pass 1 - 视频组装**：复用现有 trim + concat 逻辑 → 生成临时视频文件
2. **Pass 2 - 音频叠加**：收集所有 shot_audio → 计算绝对时间偏移 → FilterGraph 混合 → overlay 到视频

这样 Pass 1 可用 stream copy（快），Pass 2 只编码音频（视频仍 copy）。

### 1.3 音轨时间轴计算

每个 shot 在最终视频中的起始位置 = 前面所有 clip 的实际时长之和：

```
shot_1: 0ms        ~ 3000ms   (clip duration 3s)
shot_2: 3000ms     ~ 5500ms   (clip duration 2.5s)
shot_3: 5500ms     ~ 9000ms   (clip duration 3.5s)
...
```

某 shot 的某音频在最终时间轴的绝对偏移 = `shot_start_ms + audio.offset_ms`。

### 1.4 导出设置持久化

使用现有 `app_preference` 表（KV 存储），不新建 migration。

### 1.5 分辨率/码率控制

- `Source`：不重编码视频（`-c:v copy`），速度最快
- `1080p` / `720p`：需重编码（`-c:v libx264 -preset medium -crf 20 -vf scale=-2:1080`）
- 码率：`auto`（CRF 模式）或指定值（`-b:v 8M`）

---

## 2. 数据模型

### 2.1 导出设置（app_preference KV）

无新 migration。Key 约定：

| Key | 值类型 | 默认值 | 说明 |
|-----|--------|--------|------|
| `export.resolution` | string | `"source"` | "source" / "1080p" / "720p" |
| `export.bitrate` | string | `"auto"` | "auto" / "8" / "4" / "2"（Mbps） |
| `export.format` | string | `"mp4"` | "mp4" / "webm" |
| `export.include_voice` | string | `"true"` | 是否包含配音 |
| `export.include_sfx` | string | `"true"` | 是否包含音效 |
| `export.include_bgm` | string | `"true"` | 是否包含 BGM |

### 2.2 Rust 类型

```rust
// crates/core/src/export.rs (追加)

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[serde(rename_all = "camelCase")]
pub struct FinalExportSettings {
    pub resolution: ExportResolution,
    pub bitrate: ExportBitrate,
    pub format: ExportFormat,
    pub include_voice: bool,
    pub include_sfx: bool,
    pub include_bgm: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, TS)]
#[serde(rename_all = "lowercase")]
pub enum ExportResolution {
    Source,
    #[serde(rename = "1080p")]
    Hd1080,
    #[serde(rename = "720p")]
    Hd720,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[serde(rename_all = "lowercase")]
pub enum ExportBitrate {
    Auto,
    Fixed(f64), // Mbps
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, TS)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Mp4,
    Webm,
}
```

---

## 3. 核心函数

### 3.1 解析函数

```rust
// crates/core/src/export.rs

/// Resolved audio for the final timeline
pub struct ResolvedAudio {
    pub path: PathBuf,
    pub role: AudioRole,
    pub volume: f64,
    pub absolute_offset_ms: i64,  // shot_start + audio.offset_ms
}

/// Full resolution result for final export
pub struct ResolvedFinalExport {
    pub clips: Vec<ResolvedClip>,         // 视频片段（复用现有）
    pub audio_tracks: Vec<ResolvedAudio>, // 所有音频（已计算绝对偏移）
    pub total_video_duration_ms: i64,
}

/// Resolve all assets for final export
pub fn resolve_final_export(
    conn: &Connection,
    episode_id: &str,
    workspace_root: &Path,
    settings: &FinalExportSettings,
) -> Result<ResolvedFinalExport>;
```

`resolve_final_export` 逻辑：
1. 调用现有 `resolve_clips` 获取视频片段
2. 计算每个 shot 在时间轴上的起始位置
3. 遍历每个 shot 的 `shot_audio` 记录
4. 按 `settings.include_*` 过滤角色
5. 计算绝对偏移 = shot_start_ms + offset_ms
6. 验证音频文件存在

### 3.2 导出函数

```rust
/// Execute the final export (video + audio synthesis)
pub fn export_final(
    config: &FfmpegConfig,
    resolved: &ResolvedFinalExport,
    settings: &FinalExportSettings,
    output_path: &Path,
    on_progress: &mut dyn FnMut(FfmpegProgress),
) -> Result<PathBuf>;
```

`export_final` 逻辑：
1. **Pass 1**：若多 clip → trim + concat 到临时文件；单 clip → 直接使用
2. **Pass 2**：若无音频 → 直接复制/移动临时视频到 output（或根据 resolution 重编码）
3. **Pass 2 有音频时**：
   - 构建 FilterGraph：每个 audio → volume → adelay → amix
   - 构建 FFmpeg 命令：
     ```
     ffmpeg -i video.mp4 -i audio1.mp3 -i audio2.wav ...
       -filter_complex "{graph}"
       -map 0:v -map "[amixed]"
       -c:v {video_codec} -c:a aac
       {resolution_flags} {bitrate_flags}
       output.mp4
     ```
   - 执行并报告进度
4. 清理临时文件

### 3.3 文件大小估算

```rust
/// Estimate output file size in bytes
pub fn estimate_export_size(
    resolved: &ResolvedFinalExport,
    settings: &FinalExportSettings,
) -> u64;
```

估算公式：
- Source + auto bitrate → sum(input sizes)
- 指定分辨率 → duration_s × bitrate_bps / 8
- 音频部分 → duration_s × 128kbps / 8（AAC 128k）

---

## 4. IPC 命令

| command | 签名 | 说明 |
|---|---|---|
| `export_final` | `(app: AppHandle, episode_id: String, output_path: String, settings: FinalExportSettings) -> Result<String, String>` | 完整导出，带进度事件 |
| `get_export_settings` | `() -> Result<FinalExportSettings, String>` | 读取持久化设置 |
| `save_export_settings` | `(settings: FinalExportSettings) -> Result<(), String>` | 保存设置 |
| `estimate_export_size` | `(episode_id: String, settings: FinalExportSettings) -> Result<u64, String>` | 预估文件大小 |

进度事件复用现有 `FfmpegProgressTick`。

---

## 5. CLI 命令

```rust
// crates/cli/src/commands/export.rs — 追加 subcommand

/// mango export final --episode <id> --output <path> [options]
#[derive(Args)]
pub struct ExportFinalArgs {
    #[arg(long)]
    episode: String,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value = "source")]
    resolution: String,      // "source" | "1080p" | "720p"
    #[arg(long, default_value = "auto")]
    bitrate: String,         // "auto" | "8" | "4" | ...
    #[arg(long, default_value = "mp4")]
    format: String,          // "mp4" | "webm"
    #[arg(long, default_value = "true")]
    voice: bool,
    #[arg(long, default_value = "true")]
    sfx: bool,
    #[arg(long, default_value = "true")]
    bgm: bool,
}
```

CLI 输出：
```
导出中... [████████████████████░] 80% (3.2x)
✓ 导出完成: ./output.mp4 (24.5 MB, 01:32)
```

---

## 6. 前端组件

### 6.1 更新 ExportSettingsSheet

现有 `src/components/editor/ExportSettingsSheet.tsx` 追加：

```
┌─ 导出设置 ───────────────────────────┐
│                                       │
│ 分辨率:  [源分辨率 ▼]                  │
│ 码率:    [自动(CRF) ▼]               │
│ 格式:    [MP4 ▼]                      │
│                                       │
│ ─── 音频轨 ──────────────────────── │
│ ☑ 配音    ☑ 音效    ☑ 背景音乐       │
│                                       │
│ 预估大小: ~24.5 MB                    │
│                                       │
│ [取消]              [开始导出]         │
└───────────────────────────────────────┘
```

### 6.2 新增组件

```
src/components/editor/
├── ExportSettingsSheet.tsx       # 更新：追加音频/分辨率选项
├── ExportProgressOverlay.tsx    # new: 全屏进度遮罩
└── ExportCompleteDialog.tsx     # new: 完成后操作（打开文件/文件夹）
```

**ExportProgressOverlay**：
- 全屏半透明遮罩 + 居中进度卡片
- 进度条 + 百分比 + 速度 (x)
- 取消按钮
- 完成后自动切换到 ExportCompleteDialog

**ExportCompleteDialog**：
- 文件路径显示
- "打开文件" 按钮（`shell.open`）
- "打开所在文件夹" 按钮（`shell.showItemInFolder`）
- 文件大小 + 时长显示

---

## 7. 目录增量

```
crates/core/src/
└── export.rs                    # 追加 final export 逻辑（或拆为 export/ 目录）

crates/cli/src/commands/
└── export.rs                    # 追加 final subcommand

src-tauri/src/commands/
└── video.rs                     # 追加 export_final 等命令

src/components/editor/
├── ExportSettingsSheet.tsx       # 更新
├── ExportProgressOverlay.tsx    # new
└── ExportCompleteDialog.tsx     # new
```

如果 `export.rs` 因追加内容超过 300 行，拆分为：
```
crates/core/src/export/
├── mod.rs              # re-exports
├── clips.rs            # 现有 resolve_clips + export_resolved_clips
├── final_export.rs     # 新 resolve_final + export_final
├── settings.rs         # FinalExportSettings 类型
└── estimate.rs         # 大小估算
```

---

## 8. 测试

| 层级 | 用例 | 说明 |
|------|------|------|
| Rust 单元 | `resolve_final_no_clips` | 无 clip 报错 |
| Rust 单元 | `resolve_final_clips_only` | 无音频时 audio_tracks 为空 |
| Rust 单元 | `resolve_final_with_audio` | 正确解析 shot_audio + 绝对偏移 |
| Rust 单元 | `resolve_final_respects_include_flags` | 关闭 voice/sfx/bgm 时过滤对应轨 |
| Rust 单元 | `timeline_offset_calculation` | 3 个 shot 偏移累计正确 |
| Rust 单元 | `estimate_size_source_auto` | 源分辨率估算 ≈ 输入文件大小之和 |
| Rust 单元 | `estimate_size_1080p_8mbps` | 按公式计算 |
| Rust 单元 | `settings_persistence_roundtrip` | 保存后读取一致 |
| Rust 集成 | `export_final_video_only` | 无音频时等价于 v1 |
| Rust 集成 | `export_final_with_voice` | 视频+1 配音 → 输出有音频流 |
| Rust 集成 | `export_final_with_all_tracks` | 视频+配音+音效+BGM → 正确混合 |
| Rust 集成 | `export_final_resolution_720p` | 输出分辨率为 720p |
| Rust 集成 | `export_final_webm_format` | 输出为 webm 容器 |
| 前端 | `ExportSettingsSheet` | 渲染所有控件 |
| 前端 | `ExportSettingsSheet_toggles` | 音频开关更新设置 |
| 前端 | `ExportProgressOverlay` | 进度条更新 |
| 前端 | `ExportCompleteDialog` | 显示文件信息和操作按钮 |

---

## 9. 验收清单

- [ ] `export_final` 产出包含音频的可播放视频
- [ ] 音频时间与对应视频 shot 精确对齐
- [ ] shot_audio 的 volume/offset 设置在输出中生效
- [ ] 分辨率切换正常（source/1080p/720p）
- [ ] 码率设置正常（auto CRF / 固定码率）
- [ ] 格式切换正常（MP4 H.264+AAC / WebM VP9+Opus）
- [ ] 进度报告端到端工作（后端→事件→前端进度条）
- [ ] 导出设置在 session 间持久化
- [ ] 预估文件大小合理（在实际大小 2x 以内）
- [ ] CLI `mango export final` 与 GUI 导出结果一致
- [ ] CLI 进度输出到 stderr，最终路径输出到 stdout
- [ ] 无音频绑定时导出等价于 v1（纯视频）
- [ ] shot 无 adopted_asset → 跳过该 shot 并 warn
- [ ] 导出完成后可通过 UI 直接打开文件/文件夹
- [ ] `pnpm typecheck` 通过
- [ ] `cargo clippy` 无新增 warning
