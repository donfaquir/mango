# SPEC-36: 片段编排、拼接导出与 CLI

> 对应 MS5 原任务 7（片段排序 UI）+ 原任务 11 导出部分。将时间轴裁剪的片段组织为有序列表，拼接导出为最终视频。

依赖：spec-33（trim / concat 命令）、spec-34（progress 事件）、spec-35（时间轴提供裁剪参数）

非目标：
- 字幕烧录 / 气泡叠加 — MS6
- 音频混合 — MS6
- 项目文件备份 — 独立任务（非 MS5 范围）
- 分辨率转换 — 需要重编码管线，后续迭代

---

## 1. 设计决策

### 1.1 MVP 不做分辨率转换

当前 `concat_videos` 只有 Demuxer 模式（`-c copy`），不重编码。分辨率转换需要完整的重编码管线。MVP 的导出保持源视频原始参数，`ExportSettings` 不包含 `resolution` 字段。

### 1.2 `export_concat` 编排逻辑放在 core 层独立模块

导出流程需要读 DB（video_clip 列表 + asset 路径）、调 FFmpeg 命令、管理临时文件——是业务编排，不是纯 FFmpeg 操作。放在 `crates/core/src/export.rs` 作为独立业务模块，CLI 和 Tauri 都能复用。不放在 `ffmpeg/` 下。

### 1.3 单 clip 导出简化

- 0 clips → 报错
- 1 clip 无裁剪 → 直接复制源文件到输出路径（`std::fs::copy`）
- 1 clip 有裁剪 → `trim_video` 直接输出到目标路径
- 2+ clips → 各片段 trim 到临时文件 → `concat_videos` → 清理临时文件

---

## 2. 数据模型（video_clip 表）

**migration 013**：

```sql
CREATE TABLE video_clip (
    id              TEXT PRIMARY KEY NOT NULL,
    project_id      TEXT NOT NULL REFERENCES project(id),
    episode_id      TEXT REFERENCES episode(id),
    source_asset_id TEXT NOT NULL REFERENCES asset(id),
    label           TEXT,
    trim_start_ms   INTEGER,
    trim_end_ms     INTEGER,
    order_index     INTEGER NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_video_clip_episode ON video_clip(episode_id, order_index);
```

- `trim_start_ms` / `trim_end_ms`：NULL 表示不裁剪（使用全段）
- `order_index`：从 0 开始，决定拼接顺序
- `episode_id`：可选关联，便于按集组织

**Rust model**：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
pub struct VideoClip {
    pub id: String,
    pub project_id: String,
    pub episode_id: Option<String>,
    pub source_asset_id: String,
    pub label: Option<String>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub trim_start_ms: Option<i64>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub trim_end_ms: Option<i64>,
    pub order_index: i32,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Type, TS)]
pub struct CreateVideoClipInput {
    pub project_id: String,
    pub episode_id: Option<String>,
    pub source_asset_id: String,
    pub label: Option<String>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub trim_start_ms: Option<i64>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub trim_end_ms: Option<i64>,
}

#[derive(Debug, Deserialize, Type, TS)]
pub struct UpdateVideoClipInput {
    pub label: Option<String>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub trim_start_ms: Option<i64>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub trim_end_ms: Option<i64>,
}
```

---

## 3. 片段列表 UI（ClipAssembly）

**布局**：垂直列表，位于视频编辑器底部。

**每个 ClipCard**：
- 缩略图（从 `source_asset_id` 在 `trim_start_ms` 处抽取，复用 `extract_thumbnail`）
- 标签（可编辑，默认为视频文件名）
- 时长显示：`trimEnd - trimStart`（或全段时长）
- 裁剪范围文字：`0:02.0 – 0:08.0`
- 操作按钮：编辑裁剪范围、删除

**拖拽排序**：使用 `@dnd-kit/sortable`（项目已有，`ShotListPanel` 中已使用该模式）。拖拽结束后批量更新 `order_index`。

**添加片段**：
- 入口 1：从时间轴（spec-35）「添加到片段列表」按钮，传入当前 `source_asset_id` + `trimStart` + `trimEnd`
- 入口 2：片段列表底部 `+` 按钮，打开素材选择器（filter `asset_type = 'video'`），选中后以全段添加

**删除片段**：确认对话框后 DELETE 行。

---

## 4. 导出流程

「导出」按钮位于片段列表顶部工具栏。点击后弹出 `ExportSettingsSheet`（shadcn `Sheet`）：

**设置项**（MVP 精简版）：

| 设置 | 选项 | 默认 |
|---|---|---|
| 输出路径 | 文件选择器 | 项目 `exports/{episode_title}_{timestamp}.mp4` |
| 格式 | MP4 (H.264) | MP4（仅此一项，不可选） |

**执行流程**（`export_video_clips` core 函数）：

1. 从 DB 读取片段列表（按 `order_index` 排序）
2. 无 clip 时返回错误
3. 单 clip 无裁剪 → `std::fs::copy` 到输出路径
4. 单 clip 有裁剪 → `trim_video` 到输出路径
5. 多 clip → 各片段 trim 到临时文件（`{temp_dir}/mango_trim_{uuid}.mp4`）→ `concat_videos` → 清理临时文件
6. 全程通过 `on_progress` 回调报告进度
7. 完成后 Tauri 层 toast 提示 + 「打开所在文件夹」按钮

**进度计算**（多 clip 场景）：裁剪阶段（0-50%）按片段数等分 + 拼接阶段（50-100%）。

---

## 5. 页面集成

**新页面** `VideoEditorPage`：

```
路由：/project/:projectId/episodes/:episodeId/editor
入口：EpisodeDetailPage 上的「视频编辑」按钮
```

**布局**（上中下三栏）：

```
┌──────────────────────────────────────────┐
│              VideoPreview                │
│        (spec-35 VideoPreview)            │
├──────────────────────────────────────────┤
│              Timeline                    │
│  (spec-35 Timeline + TrimHandles)        │
├──────────────────────────────────────────┤
│           ClipAssembly                   │
│  [ClipCard] [ClipCard] [ClipCard] [+]    │
│                          [Export button]  │
└──────────────────────────────────────────┘
```

**视频来源选择**：进入编辑器时，从该集的已采用视频素材（`shot.adopted_asset_id`）或所有视频类型 asset 中选择要编辑的视频。

---

## 6. CLI 导出命令

```
mango export clip --project <id> --episode <id> --output <path>
```

**行为**：
1. 从 DB 读取该 episode 的 `video_clip` 列表（按 `order_index`）
2. 如无 clip 记录，报错退出
3. 执行与 GUI 相同的 `export_video_clips` 流程
4. 进度输出到 stderr（`\r` 覆写同一行）
5. 完成后输出文件路径和大小到 stdout

---

## 7. 目录增量

```
crates/core/src/
├── models/
│   └── video_clip.rs          # VideoClip, CreateVideoClipInput, etc.
├── db/queries/
│   └── video_clip.rs          # CRUD + reorder queries
└── export.rs                  # export_video_clips orchestration (独立业务模块)

src-tauri/src/commands/
└── video.rs                   # list/create/update/delete/reorder + export_video_clips

crates/cli/src/commands/
└── export.rs                  # mango export clip

src/components/editor/
├── ClipAssembly.tsx            # 片段列表容器
├── ClipCard.tsx                # 单个片段卡片
└── ExportSettingsSheet.tsx     # 导出设置面板

src/pages/
└── VideoEditorPage.tsx         # 视频编辑器页面

src/hooks/
└── useVideoClips.ts            # TanStack Query hooks for video_clip CRUD
```

`crates/core/src/models/mod.rs` 新增 `pub mod video_clip;`
`crates/core/src/db/queries/mod.rs` 新增 `pub mod video_clip;`

---

## 8. IPC 一览

| command | 签名 | 说明 |
|---|---|---|
| `list_video_clips` | `(episode_id: String) -> Result<Vec<VideoClip>, String>` | 按 order_index 升序返回 |
| `create_video_clip` | `(input: CreateVideoClipInput) -> Result<VideoClip, String>` | 创建片段，order_index 自动追加到末尾 |
| `update_video_clip` | `(id: String, input: UpdateVideoClipInput) -> Result<VideoClip, String>` | 更新裁剪范围/标签 |
| `delete_video_clip` | `(id: String) -> Result<(), String>` | 删除片段 |
| `reorder_video_clips` | `(ids: Vec<String>) -> Result<(), String>` | 按传入顺序批量更新 order_index |
| `export_video_clips` | `(episode_id: String, output_path: String) -> Result<String, String>` | 执行裁剪+拼接导出，返回输出文件路径 |

---

## 9. 测试

### 9.1 后端测试（DB + 导出逻辑）

| 用例 | 说明 |
|---|---|
| `create_clip` | 创建后 order_index 正确，DB 有记录 |
| `create_clip_auto_order` | 已有 2 个 clip，新增自动 order_index=2 |
| `delete_clip` | DELETE 后查不到 |
| `reorder_clips` | 3 个 clip 传 `[c, a, b]` 后 order_index 为 0,1,2 |
| `list_clips_ordered` | 返回按 order_index 升序 |
| `export_no_clips` | 无 clip 时返回错误 |
| `export_single_no_trim` | 单 clip 无裁剪 → 输出文件与源文件大小相同 |
| `export_single_with_trim` | 单 clip 有裁剪 → 输出时长正确 |
| `export_multi_clips` | 多 clip → 输出时长 ≈ 各段之和 |

### 9.2 前端测试

| 用例 | 说明 |
|---|---|
| `ClipCard renders` | 显示缩略图、标签、时长 |
| `ClipAssembly drag` | 拖拽后调用 reorder_video_clips |
| `ExportSettingsSheet` | 点导出调用 export_video_clips |

---

## 10. 验收清单

- [ ] migration 013 在空库和已有数据库上正常执行
- [ ] 可创建、编辑、删除片段记录
- [ ] 拖拽排序后 order_index 正确更新
- [ ] 从时间轴「添加到片段列表」带入裁剪范围
- [ ] 从素材选择器添加片段（全段）
- [ ] 单 clip 无裁剪导出 → 直接复制，文件大小一致
- [ ] 单 clip 有裁剪导出 → trim 直出，时长正确
- [ ] 多 clip 导出 → trim + concat，输出可播放 mp4
- [ ] 导出过程中显示进度条
- [ ] 导出完成后可打开文件所在目录
- [ ] 临时裁剪文件在导出完成后被清理
- [ ] CLI `mango export clip` 输出与 GUI 一致
- [ ] 无 clip 记录时导出给出明确错误提示
