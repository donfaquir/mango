# MS7 依赖一览

## 1. Rust 依赖（workspace）

| Crate | 用途 | 引入 Spec | 备注 |
|-------|------|-----------|------|
| 无新增 | — | — | 现有 serde/uuid/rusqlite/tokio 等已满足需求 |

视频 filter graph 和渲染管线通过 FFmpeg sidecar 完成（已在 MS5 集成），不引入 Rust 原生视频处理库。

## 2. npm 依赖

| Package | 用途 | 引入 Spec | 备注 |
|---------|------|-----------|------|
| 无新增 | — | — | 拖拽用现有 `@dnd-kit`，UI 用现有 shadcn 组件 |

虚拟滚动如需引入（如 `@tanstack/react-virtual`），在 spec-44 实施时确认并审批。

## 3. Tauri 插件

无新增。

## 4. 数据库 migration

| 文件 | DDL | 引入 Spec |
|------|-----|-----------|
| `015_timeline.sql` | CREATE TABLE timeline_track (id, episode_id, track_type, label, order_index, muted, locked, created_at) + index | spec-43 |
| | CREATE TABLE timeline_item (id, track_id, asset_id, item_type, position_ms, duration_ms, in_point_ms, out_point_ms, params_json, order_index, created_at) + index | spec-43 |
| | CREATE TABLE timeline_keyframe (id, item_id, property, time_ms, value, easing, created_at) + index | spec-43 |

其他 spec 无新 migration：
- spec-42: 纯 Rust 代码扩展，无 DB 变更
- spec-44: 前端 store/UI，无 DB 变更
- spec-45: probe.rs 扩展 + 导出函数，无 DB 变更

## 5. Tauri command 增量

| Command | 引入 Spec | 用途 |
|---------|-----------|------|
| `list_timeline_tracks` | spec-43 | 列出 episode 的轨道 |
| `create_timeline_track` | spec-43 | 创建轨道 |
| `delete_timeline_track` | spec-43 | 删除轨道（级联 items） |
| `reorder_timeline_tracks` | spec-43 | 重排轨道 |
| `create_default_tracks` | spec-43 | 创建 6 条默认轨道 |
| `list_timeline_items` | spec-43 | 列出轨道上的 items |
| `create_timeline_item` | spec-43 | 创建 item |
| `update_timeline_item` | spec-43 | 更新 item |
| `move_timeline_item` | spec-43 | 移动 item（跨轨道） |
| `delete_timeline_item` | spec-43 | 删除 item |
| `import_audio_from_shots` | spec-43 | MS6 音频导入到时间轴 |
| `render_preview` | spec-44 | 渲染低分辨率代理预览 |
| `export_timeline` | spec-45 | 多轨时间轴导出 |

## 6. CLI 子命令增量

| 命令 | 引入 Spec | 用途 |
|------|-----------|------|
| `mango export timeline --episode <id> --output <path> [--preset fast] [--crf 18] [--codec libx264]` | spec-45 | 多轨时间轴导出 |

现有 `mango export clip` 和 `mango export final` 保持不变。

## 7. 建议 PR 实施顺序

```
PR 1: spec-42  Filter Graph 视频扩展
│     (10-14h)
│     ├── crates/core/src/ffmpeg/filter_graph.rs  # 扩展：11 个视频 filter + normalize
│     ├── crates/core/src/ffmpeg/render.rs         # NEW: RenderConfig + render_timeline
│     └── crates/core/src/ffmpeg/mod.rs            # 追加 pub mod render
│
│                    可与 PR 1 并行
│                    ─────────────────
PR 2: spec-43  Timeline 数据模型
│     (10-14h)
│     ├── crates/core/src/db/migrations/015_timeline.sql
│     ├── crates/core/src/db/queries/timeline_*.rs  # 3 个 query 文件
│     ├── crates/core/src/models/timeline*.rs       # 2 个 model 文件
│     ├── crates/core/src/timeline.rs               # import_audio_from_shots
│     ├── src-tauri/src/commands/timeline.rs         # 11 个 IPC 命令
│     └── src-tauri/src/lib.rs                      # 注册命令
│
│          PR 2 merged 后
│          ────────────────
PR 3: spec-44  多轨 UI + Store
│     (16-22h)
│     ├── src/stores/multiTrackStore.ts
│     ├── src/hooks/useTimeline.ts
│     ├── src/components/editor/MultiTrack*.tsx      # 4 个新组件
│     ├── src/components/editor/TrackHeader.tsx
│     ├── src/components/editor/TimelineItem.tsx
│     └── src/pages/VideoEditorPage.tsx              # 修改
│
│          PR 1 + PR 2 merged 后
│          ──────────────────────
PR 4: spec-45  Render Pipeline + Export
      (8-12h)
      ├── crates/core/src/export.rs                  # 追加 export_timeline
      ├── crates/core/src/ffmpeg/probe.rs            # 追加 pixel_format
      ├── src-tauri/src/commands/video.rs             # 追加 export_timeline 命令
      ├── crates/cli/src/commands/export.rs           # 追加 timeline 子命令
      └── src/components/editor/ExportSettingsSheet.tsx  # 追加 RenderConfig UI
```

最优路径（可并行）：
- PR 1 和 PR 2 同时开发（无相互依赖）
- PR 3 在 PR 2 merged 后开始
- PR 4 在 PR 1 + PR 2 merged 后开始（需要 filter graph + timeline 数据）

## 8. 版本验证清单（实施 PR 前勾选）

- [ ] migration 编号 015 未被其他分支占用
- [ ] `tauri-specta` 生成的 TS binding 包含所有新类型（TrackType, ItemType, TimelineTrack, TimelineItem 等）
- [ ] 现有 `export_video_clips` / `export_final` IPC 未被破坏
- [ ] 现有 `video_clip` 表查询未被修改
- [ ] `filter_graph.rs` 新增方法不影响现有 `audio.rs` 调用
- [ ] `probe.rs` 的 `VideoMetadata` 新增字段有 `Option` 包裹，不影响现有解析
- [ ] 现有前端路由 `/project/:projectId/episodes/:episodeId/editor` 不变
- [ ] `@dnd-kit` 版本与 `package.json` pin 的一致
