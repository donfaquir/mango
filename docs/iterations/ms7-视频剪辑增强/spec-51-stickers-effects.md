# SPEC-51: 贴片/装饰 + 视觉特效

> 对应 MS7 任务 7。实现贴片叠加（表情贴纸）和视觉特效（调色预设），为漫剧成片增加视觉表现力。

依赖：
- spec-42（`FilterGraph::overlay()`, `colorbalance()`, `eq()` 方法已存在）
- spec-43（`TimelineItemParams::Sticker/Effect` variant 已定义）
- spec-45（`render.rs:ResolvedStickerOverlay` 已定义，`build_filter_graph()` 已处理 sticker overlay 和 color effect）
- spec-47（基础操作、右键菜单）

非目标：
- 动画贴纸（序列帧/WebM 叠加）— 首版仅静态 PNG 贴纸
- 粒子特效（雪花、樱花等）— 需要预渲染资源和循环叠加逻辑，延后
- 镜头效果（暗角、漏光、色差）— 优先级较低，延后
- 贴纸编辑器（自定义绘制）— 仅支持选择内置或导入自定义图片

---

## 1. 已有基础

### 1.1 数据模型

```rust
TimelineItemParams::Sticker {
    sticker_id: Option<String>,    // 内置贴纸 ID
    custom_path: Option<String>,   // 自定义图片路径
    animation: Option<String>,     // 留空，后续扩展
}

TimelineItemParams::Effect {
    effect_type: String,           // "colorbalance" | "eq" 等
    params: serde_json::Value,     // 具体参数
}
```

### 1.2 渲染管线

`ResolvedStickerOverlay`（render.rs:98）已定义 `image_path, x, y, start_ms, end_ms`。
`build_filter_graph()` 第 5 阶段已处理 sticker overlay（`overlay` filter）。

`ResolvedTimelineClip` 已有 `color_effect: Option<ColorEffectParams>`。
`build_filter_graph()` 第 2 阶段已处理 per-clip color effect（`colorbalance` 或 `eq` filter）。

---

## 2. 贴片系统

### 2.1 内置贴纸

**新建**：`assets/stickers/`（新目录）

首批 10 个静态 PNG 贴纸（64×64 ~ 128×128，透明背景）：

| ID | 标签 | 文件 | 类别 |
|---|---|---|---|
| `sweat_drop` | 汗滴 | `sweat_drop.png` | 表情 |
| `anger_vein` | 怒筋 | `anger_vein.png` | 表情 |
| `question_mark` | 问号 | `question_mark.png` | 表情 |
| `heart` | 爱心 | `heart.png` | 表情 |
| `sparkle_eyes` | 星星眼 | `sparkle_eyes.png` | 表情 |
| `ellipsis` | 省略号 | `ellipsis.png` | 表情 |
| `shock_lines` | 冲击线 | `shock_lines.png` | 特效 |
| `speed_lines` | 速度线 | `speed_lines.png` | 特效 |
| `light_burst` | 闪光 | `light_burst.png` | 特效 |
| `comic_frame` | 漫画框 | `comic_frame.png` | 边框 |

贴纸 PNG 需自行创建或采购。首版可用 SVG → PNG 生成简易版本。

### 2.2 贴纸注册

**新建**：`crates/core/src/stickers.rs`（~60 行）

```rust
pub struct StickerPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub category: &'static str,  // "emotion" | "effect" | "frame"
    pub filename: &'static str,  // relative to assets/stickers/
    pub default_width: u32,
    pub default_height: u32,
}

pub fn list_stickers() -> &'static [StickerPreset]
pub fn get_sticker(id: &str) -> Option<&'static StickerPreset>
pub fn resolve_sticker_path(sticker_id: &str, assets_dir: &Path) -> Result<PathBuf>
```

### 2.3 自定义贴纸

用户可导入自己的 PNG/SVG 图片作为贴纸：

- 通过 asset 系统导入图片 → 获得 `asset_id`
- `TimelineItemParams::Sticker { sticker_id: None, custom_path: Some(asset.file_path) }`
- resolve 时通过 `custom_path` 拼接绝对路径

### 2.4 resolve_timeline 对接

**文件**：`crates/core/src/export.rs:resolve_timeline()`

处理 overlay track 上的 sticker items：

1. 解析 `TimelineItemParams::Sticker`
2. 如果 `sticker_id` 有值 → `resolve_sticker_path(id, assets_dir)` 获取 PNG 路径
3. 如果 `custom_path` 有值 → 拼接 `workspace_root / custom_path`
4. 读取 sticker item 的 position 和 params（`position_x`, `position_y` 需从 params 中扩展或使用 timeline_keyframe）
5. 填充 `ResolvedStickerOverlay`

### 2.5 Sticker 位置参数

当前 `ResolvedStickerOverlay` 的 `x, y` 是 FFmpeg 表达式字符串。需要在 `TimelineItemParams::Sticker` 中扩展位置信息：

```rust
Sticker {
    sticker_id: Option<String>,
    custom_path: Option<String>,
    animation: Option<String>,
    position_x: Option<f64>,  // NEW: 0.0-1.0
    position_y: Option<f64>,  // NEW: 0.0-1.0
    scale: Option<f64>,       // NEW: 0.5-3.0, default 1.0
}
```

resolve 时：`x = "(W-w)*{position_x}"`, `y = "(H-h)*{position_y}"`。

---

## 3. 视觉特效（调色预设）

### 3.1 调色预设定义

**新建**：`crates/core/src/color_presets.rs`（~80 行）

```rust
pub struct ColorPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub effect: ColorEffectParams,  // 复用 render.rs 中的枚举
}
```

10 个内置预设：

| ID | 标签 | 类型 | 参数 |
|---|---|---|---|
| `warm` | 暖调 | Colorbalance | rs=0.1, bs=-0.1 |
| `cool` | 冷调 | Colorbalance | rs=-0.1, bs=0.1 |
| `vintage` | 怀旧 | Eq + Colorbalance | saturation=0.7, rs=0.05 |
| `bw` | 黑白 | Eq | saturation=0 |
| `high_contrast` | 高对比 | Eq | contrast=1.3, saturation=1.1 |
| `low_saturation` | 低饱和 | Eq | saturation=0.5 |
| `night` | 夜景 | Eq + Colorbalance | brightness=-0.1, bs=0.05 |
| `sunset` | 黄昏 | Colorbalance | rs=0.15, gs=0.05, bs=-0.1 |
| `dreamy` | 梦幻 | Eq | brightness=0.05, contrast=0.9, saturation=0.8 |
| `horror` | 恐怖 | Eq + Colorbalance | contrast=1.2, saturation=0.3, gs=-0.05 |

组合类型（如 vintage = Eq + Colorbalance）需要两次 filter 叠加——扩展 `ColorEffectParams` 为可组合类型，或在 resolve 阶段展开为两个 filter 调用。

**简化方案**：首版只支持 `Eq` 或 `Colorbalance` 单一类型，组合预设拆为最主要的单一效果。后续扩展。

### 3.2 调色应用方式

调色效果作为 `timeline_item`（`item_type = 'effect'`）放在 overlay track 上：

- `params_json = { "type": "Effect", "effect_type": "color_preset", "params": { "preset_id": "warm" } }`
- `position_ms` + `duration_ms` 定义生效时间范围
- 影响范围：该时间段内的所有视频 clip

### 3.3 resolve_timeline 对接

读 overlay track 上 `item_type = 'effect'` 的 items：

1. 解析 `TimelineItemParams::Effect`
2. 如果 `effect_type = "color_preset"` → 从 `params.preset_id` 查 `color_presets::get_preset()`
3. 找到该时间段覆盖的 video clip
4. 设置 clip 的 `color_effect` 字段

---

## 4. 前端

### 4.1 贴纸选择器

**新建**：`src/components/editor/StickerPicker.tsx`（~100 行）

- 网格展示内置贴纸（分 3 类 tab：表情/特效/边框）
- 每个贴纸显示缩略图 + 标签
- 点击贴纸 → 在 overlay track 上创建 sticker item，position_ms = playhead
- "导入自定义" 按钮 → 打开 AssetPickerDialog 选图片

### 4.2 调色选择器

**新建**：`src/components/editor/ColorPresetPicker.tsx`（~80 行）

- 网格展示 10 个调色预设
- 每个预设显示色块预览 + 标签
- 点击预设 → 在 overlay track 上创建 effect item
- 如果选中的是视频 clip → 效果时间范围自动对齐到 clip 的 position_ms + duration_ms

### 4.3 添加入口

**文件**：`src/components/editor/MultiTrackControls.tsx`

在 "添加媒体" 下拉菜单中增加：

- "添加贴纸" → 打开 StickerPicker
- "添加调色" → 打开 ColorPresetPicker

### 4.4 Sticker 位置调整

选中 sticker item 后，在 MultiTrackControls 区域显示 X/Y/Scale 滑块，调用 `updateItem` 修改 `params_json`。

---

## 5. 目录增量

### 新建文件

| 文件 | 行数（估） | 职责 |
|------|-----------|------|
| `crates/core/src/stickers.rs` | ~60 | 贴纸预设定义 + 路径解析 |
| `crates/core/src/color_presets.rs` | ~80 | 调色预设定义 |
| `assets/stickers/*.png` | — | 10 个内置贴纸 |
| `src/components/editor/StickerPicker.tsx` | ~100 | 贴纸选择器 |
| `src/components/editor/ColorPresetPicker.tsx` | ~80 | 调色预设选择器 |

### 修改文件

| 文件 | 变更 |
|------|------|
| `crates/core/src/lib.rs` | `pub mod stickers; pub mod color_presets;` |
| `crates/core/src/models/timeline_params.rs` | Sticker variant 新增 position_x/y/scale |
| `crates/core/src/export.rs` | resolve sticker 和 effect items |
| `src/components/editor/MultiTrackControls.tsx` | 添加贴纸/调色入口 |

---

## 6. 测试

| 用例 | 说明 |
|------|------|
| `list_stickers_returns_10` | 10 个内置贴纸 |
| `get_sticker_by_id` | 正确返回指定贴纸 |
| `resolve_sticker_path` | 内置贴纸路径正确拼接 |
| `list_color_presets_returns_10` | 10 个调色预设 |
| `warm_preset_params` | 暖调预设参数正确 |
| `resolve_sticker_overlay` | sticker item → ResolvedStickerOverlay 正确 |
| `resolve_color_effect` | effect item → clip.color_effect 正确 |
| `sticker_position_expression` | position_x=0.3 → x="(W-w)*0.3" |

---

## 7. 验收清单

- [ ] 10 个内置贴纸在选择器中显示
- [ ] 可在 overlay 轨添加贴纸并在时间轴上显示
- [ ] 贴纸位置（X/Y）和缩放可调整
- [ ] 可导入自定义图片作为贴纸
- [ ] 10 个调色预设在选择器中显示
- [ ] 可对视频片段应用调色效果
- [ ] 导出视频中贴纸正确叠加在指定位置
- [ ] 导出视频中调色效果正确渲染
- [ ] 多个特效可同时存在（不同时间段）
- [ ] `cargo test -p mango-core` 相关测试全通过
- [ ] `pnpm typecheck` 零错误
