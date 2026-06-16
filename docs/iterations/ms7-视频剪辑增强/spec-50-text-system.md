# SPEC-50: 文字系统

> 对应 MS7 任务 6。实现完整的文字系统，包括字幕、漫画气泡、花字标题、拟声词。支持样式编辑和导出渲染。

依赖：
- spec-43（`TimelineItemParams::Text` variant 已定义，text track 已支持）
- spec-42（`FilterGraph::drawtext()` 和 `overlay()` 方法已存在）
- spec-45（`render.rs:ResolvedTextOverlay` 已定义，`build_filter_graph()` 已处理文字叠加）
- spec-47（基础操作：文本预览叠加、属性面板基础）

非目标：
- 富文本编辑（内联加粗/斜体/分色）— 本 spec 每个 text item 一种样式
- 从 MS6 ASS/SRT 自动导入 — 后续增强
- 文字动画的前端实时预览 — 仅导出时渲染动画，预览显示静态文字

---

## 1. 已有基础

### 1.1 数据模型

`TimelineItemParams::Text`：
```rust
Text {
    text_type: TextType,  // Subtitle, Bubble, Fancy, Onomatopoeia
    content: String,
    style: serde_json::Value,       // 当前为 {}
    animation: Option<serde_json::Value>,  // 当前为 None
}
```

`AddTextDialog.tsx` 已支持 4 种类型选择和内容输入，但 style/animation 未暴露。

### 1.2 渲染管线

`ResolvedTextOverlay` 已定义（render.rs:87）：
```rust
pub struct ResolvedTextOverlay {
    pub text: String,
    pub fontfile: Option<String>,
    pub fontsize: u32,
    pub fontcolor: String,
    pub x: String,
    pub y: String,
    pub start_ms: i64,
    pub end_ms: i64,
}
```

`build_filter_graph()` 已用 `drawtext` filter 渲染 `ResolvedTextOverlay`。

### 1.3 前端

`AddTextDialog.tsx` 提供基础文本创建。`TimelineItemBlock.tsx` 已显示文本预览（类型图标 + 截断内容）。

---

## 2. TextStyle 结构化

### 2.1 扩展 TimelineItemParams::Text

**文件**：`crates/core/src/models/timeline_params.rs`

将 `style: serde_json::Value` 替换为具体类型：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TextStyle {
    pub font_family: Option<String>,   // default: "Noto Sans SC"
    pub font_size: Option<u32>,        // default: 48
    pub font_weight: Option<String>,   // "normal" | "bold"
    pub color: Option<String>,         // "#FFFFFF"
    pub outline_color: Option<String>, // "#000000"
    pub outline_width: Option<u32>,    // 0-5
    pub shadow: Option<bool>,
    pub position_x: Option<f64>,       // 0.0-1.0, 归一化坐标
    pub position_y: Option<f64>,       // 0.0-1.0
    pub alignment: Option<String>,     // "left" | "center" | "right"
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BubbleStyle {
    pub shape: Option<String>,         // "oval" | "rect" | "cloud" | "shout" | "whisper"
    pub tail_direction: Option<String>, // "bottom_left" | "bottom_right" | "top_left" etc.
    pub fill_color: Option<String>,    // "#FFFFFF"
    pub border_color: Option<String>,  // "#000000"
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TextAnimation {
    pub enter: Option<String>,         // "none" | "fade_in" | "pop_in" | "typewriter" | "slide_up"
    pub exit: Option<String>,          // "none" | "fade_out" | "pop_out" | "slide_down"
    pub enter_duration_ms: Option<i64>,
    pub exit_duration_ms: Option<i64>,
}
```

更新 `Text` variant：
```rust
Text {
    text_type: TextType,
    content: String,
    style: TextStyle,
    bubble: Option<BubbleStyle>,  // 仅 bubble 类型使用
    animation: Option<TextAnimation>,
}
```

### 2.2 向后兼容

现有 text items 的 `params_json` 只有 `{ content, text_type }`，没有 `style`。`TextStyle` 所有字段为 `Option`，`#[serde(default)]` 保证旧数据解析不报错。

---

## 3. 四种文字类型的渲染差异

### 3.1 字幕（Subtitle）

**定位**：画面底部居中（position_y ≈ 0.9）
**默认样式**：白字、黑色描边 2px、字号 36
**FFmpeg**：`drawtext` filter，带 `borderw` 参数实现描边

### 3.2 气泡（Bubble）

**定位**：自由定位（position_x/y 由用户拖拽设定）
**渲染方式**：前端预渲染气泡为透明 PNG → `overlay` filter 叠加
**气泡 PNG 生成**：

**新建**：`crates/core/src/text_renderer.rs`（~150 行）

使用 `resvg` crate（已在项目中被 SVG 贴纸使用，或新增依赖）生成气泡：

```rust
pub fn render_bubble(
    content: &str,
    style: &TextStyle,
    bubble: &BubbleStyle,
    output_path: &Path,
) -> Result<()>
```

1. 构建 SVG：气泡形状 + 内部文字
2. 用 resvg 渲染为 PNG
3. 输出到临时文件

**替代方案**：若不引入 resvg，用 FFmpeg `drawtext` + `drawbox` 组合实现简化气泡（矩形框 + 文字）。首版采用此方案，避免新增大依赖。

### 3.3 花字（Fancy）

**定位**：画面居中或自定义位置
**默认样式**：大字号 72、粗体、彩色描边、阴影
**FFmpeg**：`drawtext` filter + 多次叠加实现描边+阴影效果：
1. 先 drawtext 黑色文字偏移 2px（阴影层）
2. 再 drawtext 描边色文字（borderw）
3. 再 drawtext 主色文字

### 3.4 拟声词（Onomatopoeia）

**定位**：自由定位
**默认样式**：超大字号 96、粗体、手绘风格字体（如有）、倾斜
**FFmpeg**：与花字类似，额外参数 `fontsize` 更大

---

## 4. resolve_timeline 对接

**文件**：`crates/core/src/export.rs:resolve_timeline()`

当前 resolve_timeline 读 text track items 但填充 `ResolvedTextOverlay` 的逻辑可能不完整。补齐：

1. 解析 `TimelineItemParams::Text` → 读取 `style` 字段
2. 映射 `TextStyle` → `ResolvedTextOverlay`：
   - `fontsize` = `style.font_size.unwrap_or(48)`
   - `fontcolor` = `style.color.unwrap_or("#FFFFFF")`
   - `x` = 根据 `position_x` 和 `alignment` 计算 drawtext 的 x 表达式
   - `y` = 根据 `position_y` 计算
3. `start_ms` = item.position_ms
4. `end_ms` = item.position_ms + item.duration_ms

### 4.1 扩展 ResolvedTextOverlay

**文件**：`crates/core/src/ffmpeg/render.rs`

新增字段：

```rust
pub struct ResolvedTextOverlay {
    // existing fields...
    pub borderw: Option<u32>,     // outline width
    pub bordercolor: Option<String>,
    pub shadowx: Option<i32>,     // shadow offset
    pub shadowy: Option<i32>,
    pub shadowcolor: Option<String>,
}
```

`build_filter_graph()` 的 drawtext 调用需适配新字段。

---

## 5. 前端

### 5.1 文字编辑面板

**新建**：`src/components/editor/TextEditorPanel.tsx`（~180 行）

选中 text item 时在右侧显示编辑面板：

| 区域 | 控件 | 字段 |
|------|------|------|
| 内容 | Textarea | `content` |
| 类型 | Select | `text_type`（切换后重置默认样式） |
| 字体 | Select | `font_family`（系统字体列表或预设列表） |
| 字号 | Slider (12-120) | `font_size` |
| 字色 | ColorPicker | `color` |
| 粗体 | Toggle | `font_weight` |
| 描边 | Toggle + ColorPicker + Slider | `outline_color`, `outline_width` |
| 阴影 | Toggle | `shadow` |
| 对齐 | SegmentedControl | `alignment` |
| 位置 | X/Y Slider (0-1) | `position_x`, `position_y` |

气泡类型额外显示：
| 区域 | 控件 | 字段 |
|------|------|------|
| 形状 | Select | `bubble.shape` |
| 尾巴方向 | 8 方位选择器 | `bubble.tail_direction` |
| 填充色 | ColorPicker | `bubble.fill_color` |
| 边框色 | ColorPicker | `bubble.border_color` |

### 5.2 编辑面板集成

**文件**：`src/components/editor/MultiTrackEditor.tsx`

当 `selection` 中有单个 text item 时，在时间轴右侧显示 `TextEditorPanel`。

布局调整：
```
┌────────────────────────┬──────────┐
│     VideoPreview       │          │
├────────────────────────┤  Editor  │
│ Timeline               │  Panel   │
├────────────────────────┤          │
│ Controls               │          │
└────────────────────────┴──────────┘
```

### 5.3 改进 AddTextDialog

**文件**：`src/components/editor/AddTextDialog.tsx`

保持现有简洁流程（类型 + 内容），创建时填入类型对应的默认 TextStyle。用户可在创建后通过 TextEditorPanel 精调样式。

默认样式映射：
- subtitle: `{ font_size: 36, color: "#FFFFFF", outline_color: "#000000", outline_width: 2, position_y: 0.9, alignment: "center" }`
- bubble: `{ font_size: 28, color: "#000000", position_x: 0.5, position_y: 0.5 }` + `bubble: { shape: "oval", fill_color: "#FFFFFF" }`
- fancy: `{ font_size: 72, font_weight: "bold", color: "#FFD700", outline_color: "#FF4500", outline_width: 3, position_y: 0.3, alignment: "center" }`
- onomatopoeia: `{ font_size: 96, font_weight: "bold", color: "#FF0000", outline_color: "#000000", outline_width: 4, position_x: 0.5, position_y: 0.4 }`

### 5.4 预览增强

**文件**：`src/components/editor/VideoPreview.tsx`

spec-47 的文本预览叠加仅显示内容文字。本 spec 增强为应用 `TextStyle`：

- 根据 `style` 设置 CSS：font-size, color, text-shadow（描边模拟）, position
- bubble 类型显示简化气泡背景（CSS border-radius + background-color）
- 这是近似预览——导出使用 FFmpeg drawtext 是精确渲染

---

## 6. 目录增量

### 新建文件

| 文件 | 行数（估） | 职责 |
|------|-----------|------|
| `src/components/editor/TextEditorPanel.tsx` | ~180 | 文字属性编辑面板 |
| `src/components/editor/ColorPicker.tsx` | ~50 | 简易颜色选择器（可复用） |

### 修改文件

| 文件 | 变更 |
|------|------|
| `crates/core/src/models/timeline_params.rs` | TextStyle/BubbleStyle/TextAnimation 结构体 + Text variant 字段更新 |
| `crates/core/src/ffmpeg/render.rs` | ResolvedTextOverlay 新增字段 + build_filter_graph drawtext 适配 |
| `crates/core/src/export.rs` | resolve_timeline 解析 TextStyle → ResolvedTextOverlay |
| `src/components/editor/MultiTrackEditor.tsx` | 条件显示 TextEditorPanel |
| `src/components/editor/AddTextDialog.tsx` | 创建时填入默认 TextStyle |
| `src/components/editor/VideoPreview.tsx` | 文本预览应用 TextStyle CSS |

---

## 7. 测试

| 用例 | 说明 |
|------|------|
| `text_style_default_deserialize` | `{}` → TextStyle 全部字段 None |
| `text_style_full_roundtrip` | 完整样式序列化/反序列化一致 |
| `bubble_style_roundtrip` | BubbleStyle 序列化/反序列化一致 |
| `resolve_subtitle_position` | subtitle 的 position_y=0.9 → drawtext y 表达式正确 |
| `resolve_text_with_outline` | outline_width/color → drawtext borderw/bordercolor |
| `backward_compat_old_params` | 旧格式 `{"content":"X","text_type":"subtitle"}` 解析不报错 |
| `text_editor_updates_style` | 修改字号 → params_json 中 font_size 更新 |

---

## 8. 验收清单

- [ ] 4 种文字类型可创建，各有合理的默认样式
- [ ] 选中 text item 后右侧显示属性编辑面板
- [ ] 可编辑字体、字号、颜色、描边、阴影、对齐、位置
- [ ] bubble 类型可选择气泡形状和尾巴方向
- [ ] 预览窗口显示文字叠加（近似样式）
- [ ] 导出视频中字幕正确渲染（位置、颜色、描边）
- [ ] 导出视频中气泡正确渲染（drawtext + drawbox 或 overlay PNG）
- [ ] 修改样式后旧数据仍可正常解析（向后兼容）
- [ ] `cargo test -p mango-core` 文字相关测试全通过
- [ ] `pnpm typecheck` 零错误
