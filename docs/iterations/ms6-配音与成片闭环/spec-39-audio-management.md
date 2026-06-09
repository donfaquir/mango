# SPEC-39: 音效管理与分镜绑定

> 对应 MS6 原任务 5+6（音效素材库 + 分镜音效匹配 UI）。实现用户音频导入和分镜多音效绑定的数据模型与 UI。

依赖：无（可与 spec-37/38 并行开发；仅依赖 MS1 asset 导入基础）

非目标：
- 内置音效素材包 — 增大安装包，不适合 local-first 应用
- 音频波形可视化 / 编辑
- AI 音效生成
- 音频裁剪/拼接（使用外部工具处理后导入）

---

## 1. 设计决策

### 1.1 无内置音效

本项目是 local-first 桌面应用，面向个人创作者。内置音效会：
- 显著增大安装包体积（50-200MB）
- 音效版权问题复杂
- 创作者通常有自己的素材库

方案：只做用户导入 + 分类标签管理。未来可通过可选素材包下载实现。

### 1.2 shot_audio 关联表

当前 shot 表仅有 `adopted_asset_id`（视频/图片）。音频绑定需要独立表：
- 一个 shot 可有多条音频（1 配音 + N 音效 + 1 BGM）
- 每条音频有独立的 volume / offset 控制
- 通过 `audio_role` 区分用途

### 1.3 AudioRole 枚举

| Role | 说明 | 每 shot 数量 |
|------|------|-------------|
| `voice` | AI 配音 | 0-1（由 spec-38 生成后自动绑定） |
| `sfx` | 音效 | 0-N |
| `bgm` | 背景音乐 | 0-1 |

### 1.4 音频标签系统

复用 Asset 已有的 `label` 字段进行分类。约定标签：
- 导入时用户可选择/输入标签（如"环境"、"动作"、"UI"、"情感"、"BGM"）
- 不做硬编码分类，纯自由标签

---

## 2. 数据模型

### 2.1 Migration 014

```sql
-- crates/core/src/db/migrations/014_shot_audio.sql

CREATE TABLE shot_audio (
    id          TEXT PRIMARY KEY NOT NULL,
    shot_id     TEXT NOT NULL REFERENCES shot(id) ON DELETE CASCADE,
    asset_id    TEXT NOT NULL REFERENCES asset(id) ON DELETE CASCADE,
    audio_role  TEXT NOT NULL DEFAULT 'sfx' CHECK(audio_role IN ('voice','sfx','bgm')),
    volume      REAL NOT NULL DEFAULT 1.0 CHECK(volume >= 0.0 AND volume <= 2.0),
    offset_ms   INTEGER NOT NULL DEFAULT 0,
    order_index INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_shot_audio_shot ON shot_audio(shot_id, audio_role, order_index);
CREATE INDEX idx_shot_audio_asset ON shot_audio(asset_id);
```

### 2.2 Rust 模型

```rust
// crates/core/src/models/shot_audio.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, TS)]
#[serde(rename_all = "lowercase")]
pub enum AudioRole {
    Voice,
    Sfx,
    Bgm,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[serde(rename_all = "camelCase")]
pub struct ShotAudio {
    pub id: String,
    pub shot_id: String,
    pub asset_id: String,
    pub audio_role: AudioRole,
    pub volume: f64,
    pub offset_ms: i64,
    pub order_index: i64,
    pub created_at: String,
}

#[derive(Debug, Deserialize, Type, TS)]
#[serde(rename_all = "camelCase")]
pub struct CreateShotAudioInput {
    pub shot_id: String,
    pub asset_id: String,
    pub audio_role: AudioRole,
    #[serde(default = "default_volume")]
    pub volume: f64,
    #[serde(default)]
    pub offset_ms: i64,
}

#[derive(Debug, Deserialize, Type, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateShotAudioInput {
    pub volume: Option<f64>,
    pub offset_ms: Option<i64>,
    pub order_index: Option<i64>,
}

fn default_volume() -> f64 { 1.0 }
```

---

## 3. 查询层

```rust
// crates/core/src/db/queries/shot_audio.rs

pub fn create(conn: &Connection, input: CreateShotAudioInput) -> Result<ShotAudio>;
pub fn list_by_shot(conn: &Connection, shot_id: &str) -> Result<Vec<ShotAudio>>;
pub fn list_by_shot_and_role(conn: &Connection, shot_id: &str, role: AudioRole) -> Result<Vec<ShotAudio>>;
pub fn update(conn: &Connection, id: &str, input: UpdateShotAudioInput) -> Result<ShotAudio>;
pub fn delete(conn: &Connection, id: &str) -> Result<()>;
pub fn delete_by_shot_and_role(conn: &Connection, shot_id: &str, role: AudioRole) -> Result<u64>;
```

验证逻辑（在 create/update 中）：
- `volume` 范围 `[0.0, 2.0]`
- `offset_ms >= 0`
- `audio_role = 'bgm'` 时检查该 shot 是否已有 bgm（最多 1 个）
- `audio_role = 'voice'` 时检查该 shot 是否已有 voice（最多 1 个）

---

## 4. 自动绑定（配音生成结果）

spec-38 配音生成完成后，materializer 需在写入 asset 的同时，自动创建 `shot_audio` 记录：

```rust
// 在配音 task 成功后（runner 或 materializer 后处理）:
// 1. 已有 voice 绑定？→ 更新 asset_id（替换旧配音）
// 2. 无 voice 绑定？→ create shot_audio { role: Voice, volume: 1.0, offset: 0 }
```

此逻辑放在 `voice/generation.rs` 的后处理 hook 中，不耦合到 materializer trait。

---

## 5. IPC 命令

| command | 签名 | 说明 |
|---|---|---|
| `list_shot_audio` | `(shot_id: String) -> Result<Vec<ShotAudio>, String>` | 获取 shot 全部音频绑定 |
| `create_shot_audio` | `(input: CreateShotAudioInput) -> Result<ShotAudio, String>` | 新增绑定 |
| `update_shot_audio` | `(id: String, input: UpdateShotAudioInput) -> Result<ShotAudio, String>` | 修改音量/偏移 |
| `delete_shot_audio` | `(id: String) -> Result<(), String>` | 删除绑定 |

---

## 6. 前端组件

```
src/components/audio/
├── ShotAudioPanel.tsx           # 分镜音频绑定管理面板
├── AudioBindingCard.tsx         # 单条绑定卡片（名称、角色、音量、偏移）
├── AudioAssetPicker.tsx         # 音频资源选择对话框
├── AudioImportButton.tsx        # 快捷导入音频按钮
└── VolumeSlider.tsx             # 可复用的音量滑块 0-200%

src/hooks/
└── useShotAudio.ts              # 封装 CRUD + 乐观更新
```

### 6.1 ShotAudioPanel

放置位置：Shot 详情侧边栏，在图片/视频预览下方。

结构：
```
┌── 配音 ──────────────────────────┐
│ [♪ 小明-对白.mp3] 🔊━━━━━ 100%  │
│                                   │
├── 音效 ──────────────────────────┤
│ [♪ 打斗-01.wav] 🔊━━━ 80% +0ms  │
│ [♪ 脚步声.mp3]  🔊━━━ 60% +200ms│
│ [+ 添加音效]                      │
│                                   │
├── BGM ───────────────────────────┤
│ [♪ 紧张氛围.mp3] 🔊━━━ 40%      │
│ [+ 设置 BGM]                      │
└───────────────────────────────────┘
```

### 6.2 AudioAssetPicker

- 列出项目中 `asset_type = 'audio'` 的所有资源
- 支持按 label 过滤（标签筛选）
- 点击可预览播放（HTML5 audio）
- 确认后创建 shot_audio 绑定

### 6.3 AudioImportButton

- 触发文件选择对话框（filter: .mp3, .wav, .ogg, .m4a）
- 调用现有 `import_asset` IPC
- 导入完成后可选标签
- 自动打开 AudioAssetPicker 让用户确认绑定

---

## 7. 目录增量

```
crates/core/src/
├── db/
│   ├── migrations/
│   │   └── 014_shot_audio.sql
│   └── queries/
│       └── shot_audio.rs           # new
├── models/
│   └── shot_audio.rs               # new
└── voice/
    └── generation.rs               # 追加自动绑定逻辑

src-tauri/src/commands/
└── audio.rs                        # new

src/components/audio/
├── ShotAudioPanel.tsx
├── AudioBindingCard.tsx
├── AudioAssetPicker.tsx
├── AudioImportButton.tsx
└── VolumeSlider.tsx

src/hooks/
└── useShotAudio.ts
```

---

## 8. 测试

| 层级 | 用例 | 说明 |
|------|------|------|
| Rust 单元 | `create_shot_audio_valid` | 正常创建 |
| Rust 单元 | `create_shot_audio_invalid_volume` | volume 超范围拒绝 |
| Rust 单元 | `create_shot_audio_duplicate_voice` | 已有 voice 时拒绝重复创建 |
| Rust 单元 | `create_shot_audio_duplicate_bgm` | 已有 bgm 时拒绝重复创建 |
| Rust 单元 | `create_shot_audio_multiple_sfx` | sfx 允许多个 |
| Rust 单元 | `list_by_shot_ordered` | 按 role + order_index 排序 |
| Rust 单元 | `list_by_shot_and_role_filters` | 正确过滤 |
| Rust 单元 | `update_volume_clamps` | 更新后仍在有效范围 |
| Rust 单元 | `delete_removes_binding` | 删除绑定，asset 不受影响 |
| Rust 单元 | `shot_cascade_deletes_audio` | 删除 shot 级联删除绑定 |
| 前端 | `ShotAudioPanel_grouped` | 按 role 分组渲染 |
| 前端 | `AudioBindingCard_volume` | 滑块调整触发 update IPC |
| 前端 | `AudioAssetPicker_filter` | 标签筛选正常工作 |

---

## 9. 验收清单

- [ ] Migration 014 创建 shot_audio 表 + 索引
- [ ] CRUD 操作全通路
- [ ] volume 验证在 [0.0, 2.0] 范围
- [ ] voice/bgm 唯一性约束生效
- [ ] 删除 shot 级联删除 shot_audio
- [ ] 删除 asset 级联删除对应 shot_audio
- [ ] 前端 ShotAudioPanel 按角色分组显示
- [ ] 可导入音频文件并绑定到分镜
- [ ] 音量滑块实时更新
- [ ] 偏移量输入框可设置 ms 偏移
- [ ] 配音生成完成后自动创建 voice 绑定
- [ ] `pnpm typecheck` 通过
- [ ] `cargo clippy` 无新增 warning
