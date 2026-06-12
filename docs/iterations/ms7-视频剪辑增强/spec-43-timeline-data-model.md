# SPEC-43: 多轨时间轴数据模型 + 查询 + IPC

> 对应 MS7 任务 3 后端部分。建立多轨时间轴的数据库 schema、Rust 模型、查询层和 Tauri IPC 命令，以及 MS6 音频导入函数。

依赖：无（纯数据层，不依赖 filter graph 或前端）

非目标：
- 前端 UI 组件和 Zustand store — spec-44
- FFmpeg filter graph 扩展 — spec-42
- 渲染管线和导出 — spec-45
- timeline_keyframe 的 CRUD（本 spec 建表，任务 4 使用）

---

## 1. 数据库 Migration

### 1.1 migration 015_timeline.sql

```sql
-- timeline_track: 轨道
CREATE TABLE timeline_track (
    id          TEXT PRIMARY KEY NOT NULL,
    episode_id  TEXT NOT NULL REFERENCES episode(id) ON DELETE CASCADE,
    track_type  TEXT NOT NULL CHECK(track_type IN ('video','audio','text','overlay')),
    label       TEXT NOT NULL,
    order_index INTEGER NOT NULL,
    muted       INTEGER NOT NULL DEFAULT 0,
    locked      INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_timeline_track_episode ON timeline_track(episode_id, order_index);

-- timeline_item: 轨道上的元素
CREATE TABLE timeline_item (
    id            TEXT PRIMARY KEY NOT NULL,
    track_id      TEXT NOT NULL REFERENCES timeline_track(id) ON DELETE CASCADE,
    asset_id      TEXT REFERENCES asset(id),
    item_type     TEXT NOT NULL CHECK(item_type IN ('clip','text','sticker','transition','effect')),
    position_ms   INTEGER NOT NULL,
    duration_ms   INTEGER NOT NULL,
    in_point_ms   INTEGER NOT NULL DEFAULT 0,
    out_point_ms  INTEGER NOT NULL,
    params_json   TEXT NOT NULL DEFAULT '{}',
    order_index   INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_timeline_item_track ON timeline_item(track_id, position_ms);

-- timeline_keyframe: 关键帧（本 spec 建表，任务 4 使用）
CREATE TABLE timeline_keyframe (
    id         TEXT PRIMARY KEY NOT NULL,
    item_id    TEXT NOT NULL REFERENCES timeline_item(id) ON DELETE CASCADE,
    property   TEXT NOT NULL CHECK(property IN ('position_x','position_y','scale','rotation','opacity')),
    time_ms    INTEGER NOT NULL,
    value      REAL NOT NULL,
    easing     TEXT NOT NULL DEFAULT 'linear' CHECK(easing IN ('linear','ease_in','ease_out','ease_in_out')),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_timeline_keyframe_item ON timeline_keyframe(item_id, property, time_ms);
```

### 1.2 兼容性

保留 `video_clip` 表不变。MS5/MS6 的 `export_resolved_clips` / `export_final` 继续读 `video_clip` 表。多轨导出（spec-45 的 `export_timeline`）读 `timeline_*` 表。两套导出路径共存。

---

## 2. Rust 模型

### 2.1 新建 `crates/core/src/models/timeline.rs`

```rust
use serde::{Deserialize, Serialize};
use specta::Type;
use ts_rs::TS;

// === Track ===

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export)]
pub enum TrackType { Video, Audio, Text, Overlay }

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export)]
pub struct TimelineTrack {
    pub id: String,
    pub episode_id: String,
    pub track_type: TrackType,
    pub label: String,
    pub order_index: i32,
    pub muted: bool,
    pub locked: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export)]
pub struct CreateTimelineTrackInput {
    pub episode_id: String,
    pub track_type: TrackType,
    pub label: String,
}

// === Item ===

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export)]
pub enum ItemType { Clip, Text, Sticker, Transition, Effect }

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export)]
pub struct TimelineItem {
    pub id: String,
    pub track_id: String,
    pub asset_id: Option<String>,
    pub item_type: ItemType,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub in_point_ms: i64,
    pub out_point_ms: i64,
    pub params_json: String,
    pub order_index: i32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export)]
pub struct CreateTimelineItemInput {
    pub track_id: String,
    pub asset_id: Option<String>,
    pub item_type: ItemType,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub in_point_ms: Option<i64>,
    pub out_point_ms: i64,
    pub params_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export)]
pub struct UpdateTimelineItemInput {
    pub position_ms: Option<i64>,
    pub duration_ms: Option<i64>,
    pub in_point_ms: Option<i64>,
    pub out_point_ms: Option<i64>,
    pub params_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export)]
pub struct MoveTimelineItemInput {
    pub track_id: String,
    pub position_ms: i64,
}
```

### 2.2 TimelineItemParams typed enum

数据库存 JSON string，Rust 层用 tagged enum 提供类型安全：

```rust
// crates/core/src/models/timeline_params.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TimelineItemParams {
    Clip {
        ken_burns_preset: Option<String>,
    },
    Text {
        text_type: TextType,
        content: String,
        style: serde_json::Value,      // 任务 6 细化结构
        animation: Option<serde_json::Value>,
    },
    Transition {
        transition_type: String,
        duration_ms: i64,
    },
    Sticker {
        sticker_id: Option<String>,
        custom_path: Option<String>,
        animation: Option<String>,
    },
    Effect {
        effect_type: String,
        params: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextType { Subtitle, Bubble, Fancy, Onomatopoeia }
```

提供 `parse_params(json: &str) -> Result<TimelineItemParams>` 和 `serialize_params(params: &TimelineItemParams) -> Result<String>` 工具函数。

### 2.3 models/mod.rs

追加 `pub mod timeline;` 和 `pub mod timeline_params;`。

---

## 3. DB 查询

### 3.1 `crates/core/src/db/queries/timeline_track.rs`

```rust
pub fn list_by_episode(conn: &Connection, episode_id: &str) -> Result<Vec<TimelineTrack>>
pub fn get_by_id(conn: &Connection, id: &str) -> Result<TimelineTrack>
pub fn create(conn: &Connection, input: CreateTimelineTrackInput) -> Result<TimelineTrack>
pub fn delete(conn: &Connection, id: &str) -> Result<()>
pub fn reorder(conn: &Connection, ids: &[String]) -> Result<()>
pub fn update_muted(conn: &Connection, id: &str, muted: bool) -> Result<()>
pub fn update_locked(conn: &Connection, id: &str, locked: bool) -> Result<()>

/// Create default 6 tracks for a new episode timeline.
pub fn create_defaults(conn: &Connection, episode_id: &str) -> Result<Vec<TimelineTrack>>
```

`create_defaults` 创建 6 条默认轨道：

| order_index | track_type | label |
|---|---|---|
| 0 | Video | 视频 |
| 1 | Overlay | 贴片/特效 |
| 2 | Text | 文字 |
| 3 | Audio | 配音 |
| 4 | Audio | BGM |
| 5 | Audio | 音效 |

### 3.2 `crates/core/src/db/queries/timeline_item.rs`

```rust
pub fn list_by_track(conn: &Connection, track_id: &str) -> Result<Vec<TimelineItem>>
pub fn list_by_episode(conn: &Connection, episode_id: &str) -> Result<Vec<TimelineItem>>
pub fn get_by_id(conn: &Connection, id: &str) -> Result<TimelineItem>
pub fn create(conn: &Connection, input: CreateTimelineItemInput) -> Result<TimelineItem>
pub fn update(conn: &Connection, id: &str, input: UpdateTimelineItemInput) -> Result<TimelineItem>
pub fn move_item(conn: &Connection, id: &str, input: MoveTimelineItemInput) -> Result<TimelineItem>
pub fn delete(conn: &Connection, id: &str) -> Result<()>
pub fn batch_create(conn: &Connection, inputs: Vec<CreateTimelineItemInput>) -> Result<Vec<TimelineItem>>
```

`batch_create` 在事务中批量插入，用于 MS6 音频导入。

### 3.3 `crates/core/src/db/queries/timeline_keyframe.rs`

本 spec 仅建表 + 基础 CRUD，任务 4 实现完整关键帧系统时再扩展。

```rust
pub fn list_by_item(conn: &Connection, item_id: &str) -> Result<Vec<TimelineKeyframe>>
pub fn create(conn: &Connection, input: CreateTimelineKeyframeInput) -> Result<TimelineKeyframe>
pub fn delete(conn: &Connection, id: &str) -> Result<()>
```

### 3.4 queries/mod.rs

追加 `pub mod timeline_track;`、`pub mod timeline_item;`、`pub mod timeline_keyframe;`。

---

## 4. MS6 音频导入

### 4.1 `import_audio_from_shots` 函数

新增 `crates/core/src/timeline.rs`（或放在 `export.rs` 旁边）：

```rust
/// One-time import of MS6 shot_audio bindings into timeline tracks.
/// Walks video_clip table → shot_audio → creates timeline_items on audio tracks.
pub fn import_audio_from_shots(
    conn: &Connection,
    episode_id: &str,
    workspace_root: &Path,
    ffmpeg_config: &FfmpegConfig,
) -> Result<Vec<TimelineItem>>
```

逻辑复用 `export.rs:resolve_final_export()` 的遍历模式（lines 190-236）：

1. 读 `video_clip` 表获取 episode 的 clip 列表
2. probe 每个 clip 获取时长，按顺序累积 `timeline_offset`
3. 对每个 clip 的 `source_asset.shot_id`，读 `shot_audio` bindings
4. 按 `audio_role` 匹配目标轨道：
   - `Voice` → label="配音" 的轨道
   - `Bgm` → label="BGM" 的轨道
   - `Sfx` → label="音效" 的轨道
5. probe 音频文件获取时长
6. 创建 `TimelineItem`（item_type=Clip, asset_id=audio_asset_id, position_ms=timeline_offset+binding.offset_ms, duration_ms=audio_duration）
7. 调用 `batch_create` 写入

---

## 5. Tauri IPC 命令

新建 `src-tauri/src/commands/timeline.rs`：

| Command | 签名 | 说明 |
|---|---|---|
| `list_timeline_tracks` | `(episode_id) -> Vec<TimelineTrack>` | 列出 episode 的所有轨道 |
| `create_timeline_track` | `(input) -> TimelineTrack` | 创建轨道 |
| `delete_timeline_track` | `(id) -> ()` | 删除轨道（级联删除 items） |
| `reorder_timeline_tracks` | `(ids) -> ()` | 重排轨道 |
| `create_default_tracks` | `(episode_id) -> Vec<TimelineTrack>` | 创建 6 条默认轨道 |
| `list_timeline_items` | `(track_id) -> Vec<TimelineItem>` | 列出轨道上的 items |
| `create_timeline_item` | `(input) -> TimelineItem` | 创建 item |
| `update_timeline_item` | `(id, input) -> TimelineItem` | 更新 item 属性 |
| `move_timeline_item` | `(id, input) -> TimelineItem` | 移动 item（跨轨道 + 改位置） |
| `delete_timeline_item` | `(id) -> ()` | 删除 item |
| `import_audio_from_shots` | `(episode_id) -> Vec<TimelineItem>` | 一次性导入 MS6 音频 |

在 `src-tauri/src/lib.rs` 的 `collect_commands!` 中注册全部 11 个命令。

---

## 6. 目录增量

```
crates/core/src/
├── db/
│   ├── migrations/
│   │   └── 015_timeline.sql          # NEW
│   ├── queries/
│   │   ├── mod.rs                    # 修改：追加 3 个模块
│   │   ├── timeline_track.rs         # NEW
│   │   ├── timeline_item.rs          # NEW
│   │   └── timeline_keyframe.rs      # NEW
│   └── migrator.rs                   # 修改：追加 migration 015
├── models/
│   ├── mod.rs                        # 修改：追加 2 个模块
│   ├── timeline.rs                   # NEW
│   └── timeline_params.rs            # NEW
└── timeline.rs                       # NEW: import_audio_from_shots

src-tauri/src/
├── commands/
│   ├── mod.rs                        # 修改：追加 timeline 模块
│   └── timeline.rs                   # NEW
└── lib.rs                            # 修改：注册 11 个命令
```

---

## 7. 测试

| 用例 | 说明 |
|------|------|
| `create_track` | 创建轨道，验证 order_index 自动分配 |
| `create_defaults` | 6 条默认轨道正确创建（类型、标签、顺序） |
| `reorder_tracks` | 重排后 order_index 正确更新 |
| `delete_track_cascades_items` | 删除轨道时其 items 级联删除 |
| `create_item` | 创建 item，验证 position_ms/duration_ms |
| `create_item_default_in_point` | in_point_ms 默认为 0 |
| `update_item_position` | 修改 position_ms 生效 |
| `move_item_cross_track` | 跨轨道移动，track_id 和 position_ms 同时更新 |
| `delete_item` | 删除 item |
| `batch_create_items` | 事务批量创建 5 个 items |
| `list_by_episode_joins_tracks` | 跨轨道查询返回正确 items |
| `params_json_clip_roundtrip` | Clip variant 序列化/反序列化一致 |
| `params_json_text_roundtrip` | Text variant 含 TextType 正确 roundtrip |
| `params_json_transition_roundtrip` | Transition variant roundtrip |
| `params_json_invalid_rejects` | 非法 JSON 返回 ValidationError |
| `import_audio_basic` | 3 个 shot + shot_audio 导入后生成正确 timeline_items |
| `import_audio_offset` | binding.offset_ms 正确累加到 absolute position |
| `import_audio_role_mapping` | voice/sfx/bgm 分别映射到正确轨道 |
| `keyframe_crud` | 关键帧基础 CRUD |
| `delete_item_cascades_keyframes` | 删除 item 时关键帧级联删除 |
| `migration_table_count` | 运行 migration 后表数从 22 增到 25 |

---

## 8. 验收清单

- [ ] Migration 015 创建 3 个表 + 6 个索引，无 SQL 错误
- [ ] `create_default_tracks` 为新 episode 创建 6 条默认轨道
- [ ] Timeline track/item CRUD 全部工作（11 个 IPC 命令）
- [ ] 删除轨道级联删除 items，删除 item 级联删除 keyframes
- [ ] `TimelineItemParams` enum 的 5 个 variant 正确序列化/反序列化
- [ ] `import_audio_from_shots` 正确映射 shot_audio → timeline_item（位置、轨道）
- [ ] `batch_create` 在事务中执行，失败时全部回滚
- [ ] 现有 `video_clip` 表和查询不受影响
- [ ] `cargo test -p mango-core` 新增 21+ 测试全通过
- [ ] tauri-specta 生成的 TS bindings 包含所有新类型
