# SPEC-48: 关键帧动画系统 + Ken Burns 预设

> 对应 MS7 任务 4。实现关键帧动画系统，支持对 clip 的缩放/位移做时间插值动画。提供 8 个 Ken Burns 预设一键应用。

依赖：
- spec-43（`timeline_keyframe` 表已建好、CRUD 查询已实现）
- spec-42（`FilterGraph::zoompan()` 方法已存在）
- spec-45（`render_timeline()` 已支持 `KenBurnsParams`）
- spec-47（属性面板基础 UI）

非目标：
- 旋转/透明度动画 — `timeline_keyframe.property` 已支持，但 zoompan filter 只能做 zoom+pan，rotation/opacity 需额外 filter，留后续
- 关键帧曲线编辑器（贝塞尔控制点 UI）— 本 spec 仅支持 4 种预设缓动
- 镜头抖动/闪白/定格 — 原任务 4 包含，但属独立效果，延后

---

## 1. 已有基础

### 1.1 数据库

`timeline_keyframe` 表已在 migration 015 中创建：

```sql
property CHECK(property IN ('position_x','position_y','scale','rotation','opacity'))
easing CHECK(easing IN ('linear','ease_in','ease_out','ease_in_out'))
```

CRUD 查询已实现：`timeline_keyframe::list_by_item`, `create`, `delete`。

### 1.2 渲染管线

`render.rs:ResolvedTimelineClip` 已有 `ken_burns: Option<KenBurnsParams>`。`build_filter_graph()` 已处理 Ken Burns：当 clip 有 `ken_burns` 时生成 `zoompan` filter。

### 1.3 Params 模型

`TimelineItemParams::Clip { ken_burns_preset: Option<String> }` 已定义。

---

## 2. Ken Burns 预设定义

### 2.1 预设数据

**新建**：`crates/core/src/ken_burns.rs`（~100 行）

```rust
pub struct KenBurnsPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub keyframes: Vec<KenBurnsKeyframe>,
}

pub struct KenBurnsKeyframe {
    pub time_pct: f64,  // 0.0 ~ 1.0, relative to clip duration
    pub zoom: f64,      // 1.0 = no zoom
    pub x_pct: f64,     // 0.0 ~ 1.0, center of view
    pub y_pct: f64,
    pub easing: &'static str,
}
```

8 个内置预设：

| ID | 标签 | 效果 |
|---|---|---|
| `slow_zoom_in` | 缓慢推进 | scale 1.0→1.15, center, 线性 |
| `slow_zoom_out` | 缓慢拉远 | scale 1.15→1.0, center, 线性 |
| `pan_left` | 左移 | x 0.55→0.45, scale 1.05, 线性 |
| `pan_right` | 右移 | x 0.45→0.55, scale 1.05, 线性 |
| `pan_up` | 上移 | y 0.55→0.45, scale 1.05, 线性 |
| `pan_down` | 下移 | y 0.45→0.55, scale 1.05, 线性 |
| `zoom_pan_right` | 推进+右移 | scale 1.0→1.15 + x 0.45→0.55, 线性 |
| `dramatic_zoom` | 戏剧推进 | scale 1.0→1.4, center, ease_in |

### 2.2 预设 → zoompan 表达式转换

**新增函数**：`pub fn preset_to_zoompan(preset_id: &str, duration_frames: u32, fps: f64) -> Result<KenBurnsParams>`

将预设的 keyframe 序列转换为 `zoompan` filter 的 `z`, `x`, `y` 表达式：

- 两点线性插值：`'1.0+0.15*on/({total_frames}-1)'`
- 两点 ease_in：`'1.0+0.15*pow(on/({total_frames}-1),2)'`
- 多点需要 `if(between())` 拼接

### 2.3 resolve_timeline 对接

**文件**：`crates/core/src/export.rs:resolve_timeline()`

当前 `resolve_timeline` 中 clip 的 `ken_burns` 字段从 `params_json` 中读 `ken_burns_preset`，但尚未实现转换。补齐：

1. 解析 `TimelineItemParams::Clip { ken_burns_preset }`
2. 如果 `ken_burns_preset` 有值 → `preset_to_zoompan(preset_id, duration_frames, fps)`
3. 填充 `ResolvedTimelineClip::ken_burns`

---

## 3. 前端

### 3.1 Ken Burns 选择器

**新建**：`src/components/editor/KenBurnsSelector.tsx`（~80 行）

- 8 个预设按钮网格，每个显示图标 + 标签
- 图标：用方向箭头表示移动方向，放大镜表示缩放
- 点击预设 → 调用 `updateItem(itemId, { params_json: JSON.stringify({ type: "Clip", ken_burns_preset: presetId }) })`
- 已应用的预设高亮显示
- "清除" 按钮移除 Ken Burns 效果

### 3.2 集成位置

在 `TimelineItemBlock` 选中视频 clip 时，在控制栏或侧边栏显示 Ken Burns 选择器。

实现方式：在 `MultiTrackControls.tsx` 底部（或独立侧边栏），根据 selection 状态显示/隐藏。当选中单个视频 clip 时显示 `<KenBurnsSelector>`。

### 3.3 批量应用

选中多个视频 clip → 选择预设 → 批量 `updateItem`。前端遍历 selection 中 `item_type === "clip"` 的 items。

### 3.4 关键帧查询 Hook

**新建**：`src/hooks/useKeyframes.ts`（~20 行）

```typescript
export function useKeyframes(itemId: string) {
  return useQuery({
    queryKey: ["keyframes", itemId],
    queryFn: () => unwrap(commands.listKeyframes(itemId)),
    enabled: !!itemId,
  });
}
```

需新增 Tauri 命令 `list_keyframes`（见 §4）。

---

## 4. 新增 Tauri 命令

**文件**：`src-tauri/src/commands/timeline.rs`

| 命令 | 签名 | 说明 |
|------|------|------|
| `list_keyframes` | `(item_id) -> Vec<TimelineKeyframe>` | 查询 item 的关键帧 |
| `create_keyframe` | `(input) -> TimelineKeyframe` | 创建关键帧 |
| `delete_keyframe` | `(id) -> ()` | 删除关键帧 |
| `list_ken_burns_presets` | `() -> Vec<KenBurnsPresetInfo>` | 返回预设列表（前端展示用） |

后端查询函数 `timeline_keyframe::list_by_item/create/delete` 已存在，只需 Tauri 包装。

`KenBurnsPresetInfo` 为精简类型（id + label + description），不含表达式细节。

---

## 5. 目录增量

### 新建文件

| 文件 | 行数（估） | 职责 |
|------|-----------|------|
| `crates/core/src/ken_burns.rs` | ~100 | 预设定义 + zoompan 表达式生成 |
| `src/components/editor/KenBurnsSelector.tsx` | ~80 | 预设选择器 UI |
| `src/hooks/useKeyframes.ts` | ~20 | 关键帧查询 hook |

### 修改文件

| 文件 | 变更 |
|------|------|
| `crates/core/src/lib.rs` | `pub mod ken_burns;` |
| `crates/core/src/export.rs` | resolve_timeline 中实现 ken_burns_preset → KenBurnsParams 转换 |
| `src-tauri/src/commands/timeline.rs` | +4 个 Tauri 命令 |
| `src-tauri/src/lib.rs` | 注册新命令 |
| `src/components/editor/MultiTrackControls.tsx` | 选中 clip 时显示 KenBurnsSelector |

---

## 6. 测试

| 用例 | 说明 |
|------|------|
| `preset_slow_zoom_in_expr` | 预设 → zoompan 表达式正确 |
| `preset_dramatic_zoom_ease_in` | ease_in 缓动表达式含 pow() |
| `preset_pan_left_xy` | x 表达式正确，y 保持中心 |
| `preset_unknown_id_error` | 未知预设 ID → ValidationError |
| `resolve_timeline_with_preset` | clip 有 ken_burns_preset → ResolvedTimelineClip.ken_burns 非 None |
| `resolve_timeline_no_preset` | 无预设 → ken_burns 为 None |
| `batch_apply_updates_multiple` | 选中 3 个 clip 批量应用预设 |

---

## 7. 验收清单

- [ ] 8 个 Ken Burns 预设在选择器中显示
- [ ] 点击预设后 clip 的 `params_json` 正确更新
- [ ] 批量选中 clip 后可一键应用同一预设
- [ ] 已应用的预设可清除
- [ ] 导出视频中 Ken Burns 动效正确渲染
- [ ] 不同时长的 clip 应用同一预设，动画速度按比例适配
- [ ] `cargo test -p mango-core` Ken Burns 相关测试全通过
- [ ] `pnpm typecheck` 零错误
