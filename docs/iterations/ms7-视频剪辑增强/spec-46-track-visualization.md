# SPEC-46: 轨道可视化增强

> 对应 MS7 任务 3 补充（spec-44 的增量扩展）。补齐音频波形可视化、视频缩略图可靠性、全类型 item 可视化预览。

依赖：spec-44（多轨 UI 已落地）、FFmpeg sidecar 可用

非目标：
- 波形的交互式编辑（裁剪、增益调节）— 后续任务
- 贴纸/特效的精细渲染 — 任务 5-7 实现时细化
- 关键帧可视化 — 任务 4

---

## 1. 音频波形（核心新增）

### 1.1 方案选型：后端 FFmpeg showwavespic 生成波形图片

推荐理由：
- 与视频缩略图 (`extract_thumbnail_strip`) 完全同构的模式——后端生成、前端显示 `<img>`，代码一致性高
- showwavespic 一条 FFmpeg 命令输出 PNG，实现极简
- 生成高分辨率图片（50px/秒），前端 CSS 缩放处理 zoom——对漫剧场景（短音频 clip 为主）足够
- 不引入新依赖

### 1.2 后端：`crates/core/src/ffmpeg/waveform.rs`（新建）

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export)]
pub struct WaveformResult {
    pub path: String,        // PNG 文件绝对路径
    pub width: u32,
    pub height: u32,
}

pub fn extract_waveform(
    config: &FfmpegConfig,
    input: &Path,
    height: u32,       // 固定 64（轨道高度）
    px_per_sec: u32,    // 默认 50，控制水平精度
    output_dir: &Path,
) -> Result<WaveformResult>
```

FFmpeg 命令：
```
ffmpeg -y -i input.mp3 \
  -filter_complex "showwavespic=s={width}x{height}:colors=#93c5fd" \
  -frames:v 1 output.png
```

- `width` = `max(duration_seconds * px_per_sec, 100)`，需先 `probe_audio_duration` 获取时长
- 缓存机制复用 `thumbnail_strip_cache_key` 模式：`waveform_cache_key(input_path, height, px_per_sec) -> "{hash}_{height}_{px_per_sec}"`
- 缓存目录：`{workspace_root}/thumbnails/waveforms/{cache_key}/waveform.png`
- 命中缓存时直接返回已有 PNG 路径

### 1.3 Tauri command

在 `src-tauri/src/commands/ffmpeg.rs` 新增 `extract_waveform` 命令：

```rust
#[tauri::command]
#[specta::specta]
pub async fn extract_waveform(
    state: tauri::State<'_, crate::state::AppState>,
    input: String,
    height: u32,
    px_per_sec: u32,
) -> Result<ffmpeg::WaveformResult, IpcError>
```

内部逻辑与 `extract_thumbnail_strip` 一致：`FfmpegConfig::from_env()` → `spawn_blocking` → 调用 core 函数。

### 1.4 前端组件

**`src/hooks/useWaveform.ts`**（新建）— 复制 `useThumbnailStrip.ts` 模式：

```typescript
export function useWaveform(
  audioPath: string | null,
  height: number = 64,
  pxPerSec: number = 50,
) {
  return useQuery({
    queryKey: ["waveform", audioPath, height, pxPerSec],
    queryFn: () =>
      unwrap(commands.extractWaveform(audioPath!, height, pxPerSec)),
    enabled: !!audioPath,
    staleTime: Infinity,
  });
}
```

**`src/components/editor/AudioClipWaveform.tsx`**（新建）— 镜像 `VideoClipThumbnails.tsx` 结构：

```
useAsset(assetId) → file_path
  → projectRoot + "/" + file_path → audioPath
  → useWaveform(audioPath)
  → <img src={convertFileSrc(path)} className="h-full w-full object-fill" />
```

error/loading 处理：
- loading → `<Skeleton />`
- error → fallback UI（灰色背景 + Music 图标居中）
- 成功 → `<img>` 渲染波形

---

## 2. TimelineItemBlock 全类型可视化

修改 `src/components/editor/TimelineItemBlock.tsx`，按 `trackType` + `item_type` 分支渲染内容区：

| trackType | item_type | 渲染内容 |
|-----------|-----------|----------|
| video | clip | `<VideoClipThumbnails>` (已有) |
| audio | clip | `<AudioClipWaveform>` (新增) |
| text | text | 解析 `params_json` 显示 `content` 文字预览 + TextType 图标 |
| overlay | transition | 显示 `transition_type` 名称 + 转场图标 |
| overlay | sticker | 显示 sticker 图标 + 类型名称 |
| overlay | effect | 显示 `effect_type` 名称 + 特效图标 |

### 2.1 文字预览

从 `params_json` 安全解析 `content` 字段（`TimelineItemParams.Text`），截取前 20 字符显示。同时显示 `text_type` 的小标签（subtitle/bubble/fancy/onomatopoeia）。不需要新组件——在 `TimelineItemBlock` 内联实现。

安全解析：`try { JSON.parse(params_json) } catch { null }`，解析失败时回退为显示 item_type 文字。

### 2.2 Overlay item 渲染

Transition/Sticker/Effect 的可视化保持简单——显示类型名称 + 对应 Lucide 图标 + 区分色块。理由：这些类型的 item 在 MS7 任务 5-7 实现时才会实际使用，届时再精细化。现在只做到"类型可区分"。

图标映射（Lucide）：
- transition → `ArrowRightLeft`
- sticker → `Smile`
- effect → `Sparkles`
- text → `Type` (subtitle) / `MessageCircle` (bubble) / `Brush` (fancy) / `Zap` (onomatopoeia)

---

## 3. 视频缩略图可靠性修复

当前 `VideoClipThumbnails` 的 error/null 分支返回 `null`（空白），用户无法区分"没有内容"和"加载失败"。

修复：error/缺失分支改为返回 fallback UI（灰色背景 + Film 图标居中），与 `AudioClipWaveform` 的 error fallback 保持一致。

变更点：
- `VideoClipThumbnails.tsx:39` `return null` → 返回 fallback
- `VideoClipThumbnails.tsx:43` `return null` → 返回 fallback

---

## 4. 目录增量

### 新建文件

| 文件 | 行数（估） | 职责 |
|------|-----------|------|
| `crates/core/src/ffmpeg/waveform.rs` | ~120 | FFmpeg showwavespic 波形提取 + 缓存 |
| `src/hooks/useWaveform.ts` | ~15 | TanStack Query wrapper |
| `src/components/editor/AudioClipWaveform.tsx` | ~55 | 音频波形渲染组件 |

### 修改文件

| 文件 | 变更 |
|------|------|
| `crates/core/src/ffmpeg/mod.rs` | 新增 `pub mod waveform;` + re-export |
| `src-tauri/src/commands/ffmpeg.rs` | 新增 `extract_waveform` 命令（~25 行） |
| `src-tauri/src/lib.rs` | 注册 `extract_waveform` 命令 |
| `src/components/editor/TimelineItemBlock.tsx` | 全类型分支渲染 + AudioClipWaveform 引入 |
| `src/components/editor/VideoClipThumbnails.tsx` | error 分支 → fallback UI |

### ts-rs binding

运行 `cargo test -p mango-core` 后自动生成 `WaveformResult.ts` → `src/lib/bindings/types/`。

---

## 5. 测试

### Rust 单元测试（`waveform.rs` 内联 #[cfg(test)]）

| 用例 | 说明 |
|------|------|
| `cache_key_deterministic` | 同输入同 key |
| `cache_key_differs_by_height` | 不同 height 不同 key |
| `invalid_height_zero` | height=0 返回 ValidationError |
| `nonexistent_input` | 文件不存在返回 ValidationError |

### Rust 集成测试（`tests/ffmpeg_smoke.rs` 追加）

| 用例 | 说明 |
|------|------|
| `extract_waveform_basic` | 对测试音频文件生成波形 PNG |
| `extract_waveform_cache_hit` | 第二次调用命中缓存 |

### 前端测试

| 用例 | 说明 |
|------|------|
| `audio_clip_renders_waveform` | audio track 的 clip item 渲染 AudioClipWaveform |
| `text_item_shows_content` | text track item 显示 content 文本 |
| `video_clip_error_shows_fallback` | 视频缩略图错误时显示 fallback 图标 |

---

## 6. 验收清单

- [ ] 视频轨 clip 正常显示多帧缩略图
- [ ] 视频缩略图加载失败时显示 Film 图标 fallback（非空白）
- [ ] 音频轨 clip 显示蓝色波形图
- [ ] 音频波形随 zoom 缩放正确拉伸/压缩
- [ ] 音频波形加载失败时显示 Music 图标 fallback
- [ ] 波形文件缓存生效（第二次秒开）
- [ ] text 类型 item 显示文字内容预览 + 类型标签
- [ ] transition/sticker/effect 类型 item 显示类型名称 + 图标
- [ ] 各类型 item 色块颜色可区分
- [ ] `cargo clippy` 零新增 warning
- [ ] `pnpm typecheck` 零错误
- [ ] Rust 测试全通过
- [ ] 前端测试全通过
