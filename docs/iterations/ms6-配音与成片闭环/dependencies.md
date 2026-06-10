# MS6 依赖一览

## 1. Rust 依赖（workspace）

| Crate | 用途 | 引入 Spec | 备注 |
|-------|------|-----------|------|
| 无新增 | — | — | 现有 reqwest/serde/tokio 等已满足需求 |

TTS 直接扩展现有 BailianProvider（CosyVoice 走 DashScope 同一 API），无新 crate 依赖。音频处理通过 FFmpeg sidecar 完成，不引入 Rust 原生音频库。

## 2. npm 依赖

| Package | 用途 | 引入 Spec | 备注 |
|---------|------|-----------|------|
| 无新增 | — | — | 音频播放使用 HTML5 `<audio>`，UI 使用现有 shadcn 组件 |

## 3. Tauri 插件

无新增。音频文件操作通过现有 fs 权限和 shell 插件完成。

## 4. 数据库 migration

| 文件 | DDL | 引入 Spec |
|------|-----|-----------|
| `014_shot_audio.sql` | CREATE TABLE shot_audio (id, shot_id, asset_id, audio_role, volume, offset_ms, order_index, created_at) + 2 indexes | spec-39 |

其他 spec 无新 migration：
- spec-37: cosyvoice-v2 model 通过 seed 系统注册到现有 bailian provider
- spec-38: character_profile.voice_id 列已存在于 migration 001

## 5. Tauri command 增量

| Command | 引入 Spec | 用途 |
|---------|-----------|------|
| `generate_shot_voice` | spec-38 | 单分镜配音生成 |
| `generate_episode_voices` | spec-38 | 批量配音生成 |
| `preview_voice` | spec-38 | 音色试听预览 |
| `list_shot_audio` | spec-39 | 获取分镜音频绑定 |
| `create_shot_audio` | spec-39 | 新增音频绑定 |
| `update_shot_audio` | spec-39 | 修改音量/偏移 |
| `delete_shot_audio` | spec-39 | 删除绑定 |
| `probe_audio_duration` | spec-40 | 音频时长探测 |
| `check_audio_alignment` | spec-40 | 音画对齐检测 |
| `export_final` | spec-41 | 带音频完整导出 |
| `get_export_settings` | spec-41 | 读取导出设置 |
| `save_export_settings` | spec-41 | 保存导出设置 |
| `estimate_export_size` | spec-41 | 预估文件大小 |

## 6. CLI 子命令增量

| 命令 | 引入 Spec | 用途 |
|------|-----------|------|
| `mango export final --episode <id> --output <path> [--resolution] [--bitrate] [--format] [--voice] [--sfx] [--bgm]` | spec-41 | 完整成片导出 |

## 7. 建议 PR 实施顺序

```
PR 1: spec-37  TTS 接入（百炼 CosyVoice）
│     (6-8h)
│     ├── crates/core/src/provider/bailian/cosyvoice.rs
│     ├── mod.rs 路由 + types.rs + validate.rs 追加
│     └── seed cosyvoice-v2 model
│
├─────────────────────────────────────────────┐
│                                             │
PR 2: spec-39  音效管理            PR 3: spec-40  FFmpeg 音频
│     (8-10h)                      │     (10-14h)
│     ├── migration 014            │     ├── filter_graph.rs
│     ├── shot_audio CRUD          │     ├── audio.rs
│     ├── 前端 audio/*             │     ├── alignment.rs
│     └── 自动绑定 hook            │     └── IPC commands
│                                  │
│                                  │
PR 4: spec-38  角色配音生成 ◀──── PR 1 merged
│     (10-14h)
│     ├── Character voice_id 补齐
│     ├── voice/ 模块
│     ├── IPC commands
│     └── 前端 voice/*
│
└─────────────────┬────────────────┘
                  │
                  ▼
PR 5: spec-41  最终合成导出 ◀── PR 2 + PR 3 + PR 4 all merged
      (10-14h)
      ├── export_final 函数
      ├── ExportSettingsSheet 更新
      ├── ExportProgressOverlay
      ├── CLI export final
      └── 端到端验证
```

## 8. 版本验证清单（实施 PR 前勾选）

- [ ] `reqwest` 版本与 Cargo.toml workspace 中 pin 的一致
- [ ] FFmpeg sidecar 路径配置无变更
- [ ] `tauri-specta` 生成的 TS binding 无类型冲突
- [ ] 现有 `export_video_clips` IPC 未被破坏（保持向后兼容）
- [ ] seed 系统 `ON CONFLICT DO UPDATE` 不影响已有 provider/model（wan2.7/happyhorse）
- [ ] migration 编号 014 未被其他分支占用
