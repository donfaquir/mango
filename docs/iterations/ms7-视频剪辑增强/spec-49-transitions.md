# SPEC-49: 转场库

> 对应 MS7 任务 5。实现视频片段之间的转场效果，提供 8 种漫剧常用转场类型。

依赖：
- spec-42（`FilterGraph::xfade()` 方法已存在）
- spec-43（`TimelineItemParams::Transition` variant 已定义、timeline_item 表支持 item_type='transition'）
- spec-45（`render.rs:build_filter_graph()` 已处理 `ResolvedTransition`、normalize 保证输入同参数）
- spec-47（右键菜单、吸附对齐基础）

非目标：
- 音频交叉淡化（acrossfade）— 现有 audio track 独立处理，转场仅影响视频
- 自定义转场表达式 — 仅预设转场，不支持用户自定义 xfade 参数
- 转场缩略图预览 — 需抽帧合成，工作量大，延后

---

## 1. 已有基础

### 1.1 Filter Graph

`FilterGraph::xfade(input_a, input_b, transition, duration_secs, offset_secs, output)` 已实现（filter_graph.rs:192）。

### 1.2 渲染管线

`build_filter_graph()` 的第 3 阶段（join clips）已处理 `ResolvedTransition`：当有 transition 时使用 xfade 连接相邻 clip，否则使用 concat。

### 1.3 数据模型

`TimelineItemParams::Transition { transition_type: String, duration_ms: i64 }` 已定义。

---

## 2. 转场类型定义

### 2.1 内置转场

**新建**：`crates/core/src/transitions.rs`（~60 行）

```rust
pub struct TransitionPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub xfade_name: &'static str,  // FFmpeg xfade transition name
}
```

8 种预设：

| ID | 标签 | FFmpeg xfade 名称 | 风格 |
|---|---|---|---|
| `dissolve` | 叠化 | `dissolve` | 通用 |
| `fade_black` | 淡入黑场 | `fade` | 通用 |
| `fade_white` | 淡入白场 | `fadewhite` | 通用 |
| `wipe_left` | 左擦除 | `wipeleft` | 通用 |
| `wipe_right` | 右擦除 | `wiperight` | 通用 |
| `circle_open` | 圆形展开 | `circleopen` | 漫画风 |
| `circle_close` | 圆形收缩 | `circleclose` | 漫画风 |
| `pixelize` | 像素化 | `pixelize` | 风格化 |

提供 `pub fn list_presets() -> &'static [TransitionPreset]` 和 `pub fn get_preset(id: &str) -> Option<&'static TransitionPreset>`。

---

## 3. 转场放置逻辑

### 3.1 数据模型约束

转场 item 放在 **overlay track** 上（`item_type = 'transition'`），在两个相邻视频 clip 之间：

- `position_ms` = 前一个 clip 的 `end_ms - transition.duration_ms / 2`（居中对齐交叠区域）
- `duration_ms` = 转场时长（默认 500ms，范围 100-2000ms）
- `params_json` = `{ "type": "Transition", "transition_type": "dissolve", "duration_ms": 500 }`

### 3.2 resolve_timeline 对接

**文件**：`crates/core/src/export.rs:resolve_timeline()`

当前 `resolve_timeline` 从 overlay track 读 transition items，填充 `ResolvedTransition`。需确认：

1. transition 的 `transition_type` → 查 `transitions.rs` 获取 `xfade_name`
2. transition 的 `position_ms` 确定它属于哪两个相邻 clip
3. `duration_ms` 转换为 `duration_secs`（xfade 参数）

当前实现可能需要补充 transition → clip 配对逻辑。xfade 要求知道它在哪两个 clip 之间——通过 position_ms 与 video clip 列表做区间匹配。

---

## 4. 前端

### 4.1 转场选择器

**新建**：`src/components/editor/TransitionPicker.tsx`（~80 行）

网格式选择器，显示 8 种转场的图标和标签。每种转场配一个 Lucide 图标：

| 转场 | 图标 |
|------|------|
| dissolve | `Blend` |
| fade_black / fade_white | `SunMedium` / `Sun` |
| wipe_left / wipe_right | `ArrowLeft` / `ArrowRight` |
| circle_open / circle_close | `Circle` / `CircleDot` |
| pixelize | `Grid3X3` |

点击后创建 transition item。

### 4.2 添加转场入口

两种方式：

**方式 A：clip 之间的 "+" 按钮**

**文件**：`src/components/editor/MultiTrackTimeline.tsx`

在视频轨道上，检测相邻两个 clip 之间的间隙。在间隙处显示一个小 "+" 图标（hover 时浮现）。点击弹出 `TransitionPicker`。

**方式 B：右键菜单**

**文件**：`src/components/editor/ItemContextMenu.tsx`（spec-47）

选中视频 clip 右键 → "添加转场（后方）" → 弹出 `TransitionPicker`。

推荐先实现方式 B（改动更小），方式 A 作为后续增强。

### 4.3 转场时长调整

选中 transition item 后，在 `MultiTrackControls` 区域显示时长滑块（100ms-2000ms），调用 `updateItem` 修改 `duration_ms` 和 `params_json`。

### 4.4 Store action

**文件**：`src/stores/multiTrackStore.ts`

新增 `addTransition(afterClipId: string, transitionType: string, durationMs: number): Promise<void>`：

1. 找到 afterClip 和下一个 clip
2. 计算 position_ms（前 clip 结束 - duration/2）
3. 找到 overlay track（没有则创建）
4. `addItem({ track_id, item_type: 'transition', position_ms, duration_ms, params_json })`

---

## 5. 目录增量

### 新建文件

| 文件 | 行数（估） | 职责 |
|------|-----------|------|
| `crates/core/src/transitions.rs` | ~60 | 转场预设定义 |
| `src/components/editor/TransitionPicker.tsx` | ~80 | 转场选择器 UI |

### 修改文件

| 文件 | 变更 |
|------|------|
| `crates/core/src/lib.rs` | `pub mod transitions;` |
| `crates/core/src/export.rs` | resolve_timeline 转场→clip 配对逻辑完善 |
| `src/stores/multiTrackStore.ts` | +addTransition action |
| `src/components/editor/ItemContextMenu.tsx` | +"添加转场" 菜单项 |
| `src/components/editor/MultiTrackControls.tsx` | 选中 transition 时显示时长滑块 |

---

## 6. 测试

| 用例 | 说明 |
|------|------|
| `list_presets_returns_8` | 8 种预设全部返回 |
| `get_preset_dissolve` | 正确返回 dissolve 预设 |
| `get_preset_unknown_none` | 未知 ID → None |
| `resolve_transition_between_clips` | 转场正确配对到相邻 clip |
| `resolve_transition_xfade_name` | transition_type → xfade_name 映射正确 |
| `add_transition_store` | addTransition 创建 item 且 position_ms 正确 |
| `transition_duration_update` | 修改时长后 params_json 同步更新 |

---

## 7. 验收清单

- [ ] 8 种转场类型在选择器中显示
- [ ] 可在两个相邻视频 clip 之间添加转场
- [ ] 转场 item 在时间轴上显示为橙色块 + 类型图标
- [ ] 转场时长可通过滑块调整（100ms-2000ms）
- [ ] 转场可删除（右键菜单或 Delete 键）
- [ ] 导出视频中转场效果正确渲染
- [ ] 无转场时片段之间为硬切（不影响现有行为）
- [ ] `cargo test -p mango-core` 转场相关测试全通过
- [ ] `pnpm typecheck` 零错误
