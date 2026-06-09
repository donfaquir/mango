# SPEC-38: 角色配音生成

> 对应 MS6 原任务 3+4（角色声音绑定 + 配音生成流程）。为角色绑定固定音色，实现从分镜对白到配音音频的完整生成流程。

依赖：spec-37（TTS Provider 可用）

非目标：
- 语音克隆（从用户音频样本生成自定义音色）
- 实时流式播放（生成过程中边生成边播放）
- 音频波形编辑
- 多语言支持（首版仅中文）
- 一个分镜多角色拆分（依赖语义分析，P2）

---

## 1. 设计决策

### 1.1 voice_id 字段现状

数据库 `character_profile` 表在 migration 001 中**已包含** `voice_id TEXT` 字段。当前问题是 Rust `Character` struct 和查询层未映射该字段。本 spec 只需补齐 Rust 层，无需新 migration。

### 1.2 配音生成策略

- 单分镜生成：用户点击特定分镜的"生成配音"按钮
- 批量生成：用户点击"为全集生成配音"，系统自动：
  1. 遍历 episode 下所有 shot
  2. 筛选：`dialogue` 非空 + 关联 character 有 `voice_id`
  3. 构建 `CreateGenerationTaskInput` 列表
  4. 调用 `submit_tasks_batch` 提交

### 1.3 结果关联

生成的音频 Asset 设置：
- `asset_type = 'audio'`
- `source = 'generated'`
- `label = 'voice'`
- `shot_id = 对应分镜 id`

后续 spec-39 会将此 asset 自动绑定到 `shot_audio` 表（`audio_role = 'voice'`）。

### 1.4 试听预览

选择音色时提供"试听"按钮，使用固定示例文本或用户输入文本快速生成一段音频预览。复用 `submit_task` 接口，前端监听 task 完成事件后用 HTML5 `<audio>` 播放。

---

## 2. 数据模型变更

### 2.1 Character struct 补齐

```rust
// crates/core/src/models/character.rs
pub struct Character {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub appearance_prompt: String,
    pub reference_image_path: Option<String>,
    pub voice_id: Option<String>,        // 新增
    pub created_at: String,
    pub updated_at: String,
}
```

### 2.2 Input 类型

```rust
// CreateCharacterInput — 新增字段
pub voice_id: Option<String>,

// UpdateCharacterInput — 新增字段（三态：None=不变, Some(None)=清空, Some(Some(v))=设置）
pub voice_id: Option<Option<String>>,
```

### 2.3 查询层

```rust
// crates/core/src/db/queries/character.rs
const SELECT_COLUMNS: &str = "id, project_id, name, description, appearance_prompt, \
                              reference_image_path, voice_id, created_at, updated_at";
//                            ^ 新增 voice_id

// map_row 追加：
// voice_id: row.get("voice_id")?,
```

---

## 3. Voice 模块

```
crates/core/src/voice/
├── mod.rs             # pub mod generation;
└── generation.rs      # 配音任务构建逻辑
```

### 3.1 核心函数

```rust
// crates/core/src/voice/generation.rs

/// Build a TTS task input for a single shot
pub fn build_shot_voice_task(
    conn: &Connection,
    shot_id: &str,
    provider_id: &str,
    model_id: &str,
    account_id: &str,
) -> Result<Option<CreateGenerationTaskInput>>

/// Build TTS task inputs for all shots in an episode
pub fn build_episode_voice_tasks(
    conn: &Connection,
    episode_id: &str,
    provider_id: &str,
    model_id: &str,
    account_id: &str,
) -> Result<Vec<CreateGenerationTaskInput>>
```

`build_shot_voice_task` 逻辑：
1. 读取 shot → 获取 dialogue 字段
2. 若 dialogue 为空 → 返回 None
3. 通过 shot_character 关联 → 获取第一个角色的 voice_id
4. 若 voice_id 为空 → 返回 None
5. 构建 `CreateGenerationTaskInput`:
   - `task_type: TaskKind::Audio`
   - `provider_id`, `model_id`, `account_id`
   - `params_json: { voice_id, text: dialogue, speed: 1.0 }`
   - `shot_id: Some(shot_id)`
   - `project_id`

`build_episode_voice_tasks` = 遍历 episode 所有 shot → 调用 `build_shot_voice_task` → filter None。

---

## 4. IPC 命令

| command | 签名 | 说明 |
|---|---|---|
| `generate_shot_voice` | `(shot_id: String, account_id: String) -> Result<String, String>` | 提交单 shot TTS 任务，返回 task_id |
| `generate_episode_voices` | `(episode_id: String, account_id: String) -> Result<BatchOutcome, String>` | 批量提交全集配音 |
| `preview_voice` | `(voice_id: String, text: String, account_id: String) -> Result<String, String>` | 试听预览，返回 task_id |

```rust
// src-tauri/src/commands/voice.rs

#[tauri::command]
#[specta::specta]
pub async fn generate_shot_voice(
    state: tauri::State<'_, AppState>,
    shot_id: String,
    account_id: String,
) -> Result<String, String> {
    let input = voice::generation::build_shot_voice_task(
        &state.db, &shot_id, "bailian", "cosyvoice-v2", &account_id,
    ).map_err(|e| e.to_string())?
     .ok_or_else(|| "该分镜无对白或角色未配置音色".to_string())?;

    state.task_engine.submit(input).await.map_err(|e| e.to_string())
}
```

---

## 5. 前端组件

```
src/components/voice/
├── VoiceSelect.tsx              # 音色下拉选择器
├── VoicePreviewButton.tsx       # 试听按钮（播放预览音频）
└── VoiceGenerationPanel.tsx     # 批量配音控制面板

src/hooks/
└── useVoiceGeneration.ts        # 封装生成 + 进度监听逻辑
```

### 5.1 VoiceSelect

- 从 cosyvoice-v2 model 的 capabilities 获取预设音色列表（seed 时写入）
- 显示：音色名称 + 性别标签 + 风格描述
- 选中后更新 character.voice_id
- 含内联 VoicePreviewButton

### 5.2 VoicePreviewButton

- 点击 → 调用 `preview_voice` IPC
- 监听 task 完成事件 → 获取 result_asset_id → 读取文件路径
- 使用 `<audio>` 标签播放
- 生成中显示 loading spinner

### 5.3 VoiceGenerationPanel

放置位置：episode 详情页侧边栏或工具栏

- "为本集生成配音" 按钮
- 统计显示：共 N 个分镜，M 个可生成（有对白+有音色）
- 生成中：进度列表（每个 shot 一行状态）
- 生成完成：播放按钮快速试听

### 5.4 CharacterForm 修改

在 `src/components/subjects/forms/` 中已有 CharacterForm：
- 在 reference_image 区块后新增"配音音色"区块
- 包含 VoiceSelect + VoicePreviewButton

---

## 6. 目录增量

```
crates/core/src/
└── voice/
    ├── mod.rs
    └── generation.rs

src-tauri/src/commands/
└── voice.rs                    # new

src/components/voice/
├── VoiceSelect.tsx
├── VoicePreviewButton.tsx
└── VoiceGenerationPanel.tsx

src/hooks/
└── useVoiceGeneration.ts
```

---

## 7. 测试

| 层级 | 用例 | 说明 |
|------|------|------|
| Rust 单元 | `character_crud_with_voice_id` | create/get/update/clear voice_id |
| Rust 单元 | `build_shot_voice_task_no_dialogue` | 无对白返回 None |
| Rust 单元 | `build_shot_voice_task_no_voice_id` | 角色无 voice_id 返回 None |
| Rust 单元 | `build_shot_voice_task_success` | 有对白+有 voice_id → 正确 input |
| Rust 单元 | `build_episode_voices_mixed` | 部分 shot 有/无对白，正确过滤 |
| Rust 单元 | `build_episode_voices_multi_character` | 多角色各用自己的 voice_id |
| 前端 | `VoiceSelect` | 渲染音色选项列表 |
| 前端 | `VoicePreviewButton` | 点击触发 preview_voice IPC |
| 前端 | `VoiceGenerationPanel` | 显示可生成数量统计 |
| 前端 | `CharacterForm` | voice 区块正确显示 |

---

## 8. 验收清单

- [ ] Character struct 包含 voice_id（CRUD 全通路）
- [ ] 前端角色编辑页可选择/清除音色
- [ ] 试听按钮播放预览音频
- [ ] 单分镜配音生成 → 音频 asset 关联到 shot
- [ ] 批量生成 → 跳过无对白/无音色的 shot
- [ ] 生成的 asset: type=audio, source=generated, label=voice
- [ ] 批量生成进度在前端实时显示
- [ ] 现有 Character 相关测试不受影响
- [ ] `pnpm typecheck` 通过
- [ ] `cargo clippy` 无新增 warning
