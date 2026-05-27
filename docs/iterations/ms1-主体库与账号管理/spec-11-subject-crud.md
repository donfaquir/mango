# SPEC-11: 主体 CRUD 通用骨架（Character / Scene / Prop / Costume）

## 概述

实现四类主体（角色 / 场景 / 道具 / 服装）的 core 层 CRUD、Tauri command 层、TanStack Query hooks、以及角色 CLI 命令。Character 作为模板首先落地，Scene/Prop 复用相同模式（按文件复制，不抽象 trait），Costume 因 FK 关联 Character 单独处理级联策略。

对应 `开发任务.md` 任务 1-4 + 任务 11 中的 character 部分。

## 设计目标 / 非目标

**目标**：
- 四类主体的全栈 CRUD（core + Tauri + 前端 hooks）
- list 接口强制按 project_id 过滤；Costume 额外支持 character_id 过滤
- 每类主体一份 model 文件、一份 query 文件、一份 hooks 文件，与既有 project.rs 风格一致
- 删除 Character 时级联删除其 Costume（DB 已配置 `ON DELETE CASCADE`）
- 暴露 `mango character list/create/update/delete` CLI 命令

**非目标**：
- 不抽象通用 CRUD trait/macro。每实体一份代码，接受可控重复换取可读性与编译期类型检查
- 不暴露 `voice_id`（schema 保留，留给 MS6）
- 不实现 Scene/Prop/Costume 的 CLI 命令（V2）
- 不在本 spec 处理参考图导入与预览（→ spec-12 / spec-14）
- 不实现 asset 反向引用清理（删除一个 character 不会自动把指向其参考图的 `asset` 行变为悬挂；见 §"已知限制"）

## 技术方案

### 目录/文件结构

```
crates/core/src/
├── models/
│   ├── mod.rs                  # 添加 pub mod {character, scene, prop, costume};
│   ├── character.rs
│   ├── scene.rs
│   ├── prop.rs
│   └── costume.rs
└── db/queries/
    ├── mod.rs                  # 添加四个新模块
    ├── character.rs
    ├── scene.rs
    ├── prop.rs
    └── costume.rs

src-tauri/src/commands/
├── mod.rs                      # 注册四个新模块
├── character.rs
├── scene.rs
├── prop.rs
└── costume.rs

src/hooks/
├── useCharacters.ts
├── useScenes.ts
├── useProps.ts
└── useCostumes.ts

crates/cli/src/commands/
└── character.rs                # mango character ...
```

### 数据模型（Character 为模板）

`crates/core/src/models/character.rs`：
```rust
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Character {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub appearance_prompt: String,
    pub reference_image_path: Option<String>,
    // voice_id 存在于 schema，MS1 不暴露
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Type)]
pub struct CreateCharacterInput {
    pub project_id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub appearance_prompt: Option<String>,
    #[serde(default)]
    pub reference_image_path: Option<String>,
}

#[derive(Debug, Deserialize, Type)]
pub struct UpdateCharacterInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance_prompt: Option<String>,
    /// None = 不修改, Some(None) = 清空为 NULL, Some(Some(v)) = 设为 v
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_image_path: Option<Option<String>>,
}

#[derive(Debug, Deserialize, Type)]
pub struct ListCharactersOptions {
    pub project_id: String,                        // 必填
    #[serde(default)]
    #[specta(type = Option<specta_typescript::Number>)]
    pub limit: Option<i64>,
    #[serde(default)]
    #[specta(type = Option<specta_typescript::Number>)]
    pub offset: Option<i64>,
}
```

> **设计说明**：
> - `ListCharactersOptions.project_id` 必填，与 `ListProjectsOptions` 不同（项目列表无父级，主体列表必须有项目上下文）。
> - `reference_image_path` 用 `Option<Option<String>>` 表达三态——MS1 任务 6 详情页需要"清空参考图"操作。
> - `voice_id` 字段不出现在 model 里。Rust 侧 SELECT 时跳过该列；后续 MS6 加回时只需扩展 model + 修改 SELECT，无需 schema 迁移。

### 模型差异速查

| 实体 | 字段差异 | List 过滤 |
|---|---|---|
| Character | + appearance_prompt | project_id |
| Scene | + environment_prompt | project_id |
| Prop | （无 prompt 字段） | project_id |
| Costume | + character_id (FK)、无 prompt | project_id + 可选 character_id |

Costume 的 `character_id` 在 Create 时必填、Update 时不可改（如需迁移服装到另一角色，按删除+新建处理）。

### Costume 输入类型

```rust
#[derive(Debug, Deserialize, Type)]
pub struct CreateCostumeInput {
    pub project_id: String,
    pub character_id: String,         // 必填，且必须属于同一 project_id
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub reference_image_path: Option<String>,
}

#[derive(Debug, Deserialize, Type)]
pub struct UpdateCostumeInput {
    // 注意：character_id 不可改
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_image_path: Option<Option<String>>,
}

#[derive(Debug, Deserialize, Type)]
pub struct ListCostumesOptions {
    pub project_id: String,
    #[serde(default)]
    pub character_id: Option<String>,    // 可选过滤
    #[serde(default)]
    #[specta(type = Option<specta_typescript::Number>)]
    pub limit: Option<i64>,
    #[serde(default)]
    #[specta(type = Option<specta_typescript::Number>)]
    pub offset: Option<i64>,
}
```

### 查询实现（Character 为模板）

`crates/core/src/db/queries/character.rs`：
```rust
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::models::character::{
    Character, CreateCharacterInput, ListCharactersOptions, UpdateCharacterInput,
};

pub fn create(conn: &Connection, input: CreateCharacterInput) -> Result<Character> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(CoreError::Validation("name cannot be empty".into()));
    }
    if input.project_id.is_empty() {
        return Err(CoreError::Validation("project_id is required".into()));
    }

    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO character_profile
            (id, project_id, name, description, appearance_prompt, reference_image_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            id,
            input.project_id,
            name,
            input.description.unwrap_or_default(),
            input.appearance_prompt.unwrap_or_default(),
            input.reference_image_path,
        ],
    )?;
    // created_at / updated_at 由 schema DEFAULT (datetime('now')) 自动填入
    get_by_id(conn, &id)
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<Character> {
    conn.query_row(
        "SELECT id, project_id, name, description, appearance_prompt,
                reference_image_path, created_at, updated_at
         FROM character_profile WHERE id = ?1",
        params![id],
        |row| Ok(Character {
            id: row.get(0)?,
            project_id: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            appearance_prompt: row.get(4)?,
            reference_image_path: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        }),
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound {
            entity: "character",
            id: id.to_string(),
        },
        other => CoreError::Sqlite(other),
    })
}

pub fn list(conn: &Connection, opts: ListCharactersOptions) -> Result<Vec<Character>> {
    // 与 project::list 对齐：默认 50，上限 200
    let limit = opts.limit.unwrap_or(50).clamp(1, 200);
    let offset = opts.offset.unwrap_or(0).max(0);

    let mut stmt = conn.prepare(
        "SELECT id, project_id, name, description, appearance_prompt,
                reference_image_path, created_at, updated_at
         FROM character_profile
         WHERE project_id = ?1
         ORDER BY created_at DESC
         LIMIT ?2 OFFSET ?3",
    )?;
    let rows = stmt.query_map(
        params![opts.project_id, limit, offset],
        |row| Ok(Character {
            id: row.get(0)?,
            project_id: row.get(1)?,
            name: row.get(2)?,
            description: row.get(3)?,
            appearance_prompt: row.get(4)?,
            reference_image_path: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        }),
    )?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(CoreError::from)
}

pub fn update(conn: &Connection, id: &str, input: UpdateCharacterInput) -> Result<Character> {
    // 与 project::update 同模式：动态拼接 SET 子句
    let mut sets: Vec<&'static str> = Vec::new();
    let mut values: Vec<rusqlite::types::Value> = Vec::new();

    if let Some(name) = input.name {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(CoreError::Validation("name cannot be empty".into()));
        }
        sets.push("name = ?");
        values.push(name.into());
    }
    if let Some(desc) = input.description {
        sets.push("description = ?");
        values.push(desc.into());
    }
    if let Some(prompt) = input.appearance_prompt {
        sets.push("appearance_prompt = ?");
        values.push(prompt.into());
    }
    if let Some(ref_path) = input.reference_image_path {
        sets.push("reference_image_path = ?");
        values.push(match ref_path {
            None => rusqlite::types::Value::Null,
            Some(s) => s.into(),
        });
    }

    if sets.is_empty() {
        return get_by_id(conn, id);
    }
    // Schema 的 DEFAULT 仅在 INSERT 时生效，UPDATE 必须手动维护 updated_at
    sets.push("updated_at = datetime('now')");

    let sql = format!("UPDATE character_profile SET {} WHERE id = ?", sets.join(", "));
    values.push(id.to_string().into());

    let n = conn.execute(&sql, rusqlite::params_from_iter(values))?;
    if n == 0 {
        return Err(CoreError::NotFound {
            entity: "character",
            id: id.to_string(),
        });
    }
    get_by_id(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> Result<()> {
    let n = conn.execute(
        "DELETE FROM character_profile WHERE id = ?1",
        params![id],
    )?;
    if n == 0 {
        return Err(CoreError::NotFound {
            entity: "character",
            id: id.to_string(),
        });
    }
    Ok(())
}
```

Scene/Prop 几乎完全相同的代码，只需替换字段名与表名。Costume 的 `list` 多一个可选 `character_id` 参数，且 `create` 必须先校验 `character_id` 在同一 `project_id` 下存在（防止跨项目串数据）。

### Costume 的级联策略

Schema 已配置：
```sql
character_id TEXT NOT NULL REFERENCES character_profile(id) ON DELETE CASCADE
```

**MS1 决策**：保留级联删除。理由：
- 服装强依附于角色，角色不存则服装无意义
- 用户在 UI 删除角色时由前端弹确认框，**前端在弹窗时单独调** `useCostumeListByCharacter(projectId, characterId)` 拿到 count 展示"此角色有 N 件服装将一并删除"
- `delete_character` IPC 保持 `Result<(), IpcError>`（无返回 body）——避免 IPC schema 仅为统计目的承担额外字段；UI 用既有 hook 自给自足，无需返回值

### Costume 的跨项目防护

`create_costume` 校验 `character_id` 与 `project_id` 一致：
```rust
let row_count: i64 = conn.query_row(
    "SELECT COUNT(*) FROM character_profile WHERE id = ?1 AND project_id = ?2",
    params![input.character_id, input.project_id],
    |r| r.get(0),
)?;
if row_count == 0 {
    return Err(CoreError::Validation(
        "character_id does not belong to project_id".into(),
    ));
}
```

> **设计说明**：这是 TOCTOU 检查——`SELECT COUNT` 与紧接其后的 `INSERT` 之间，理论上有人可能删除该 character。但实际场景（单用户桌面应用）下不会触发；引入显式事务包裹两条语句解决，但收益不足以抵消代码复杂度。本 spec 接受此风险。

### 已知限制：asset 反向引用

`character.reference_image_path` 是 `asset.file_path` 的逻辑外键，但 DB 层未声明 FK 约束。删除一个 character 不会清理 asset 表；导入新参考图覆盖旧路径时，旧 asset 行也不会被自动删。

MS1 不处理：
- asset 的孤儿清理留给 V2 GC（与 spec-12 §"错误场景"中孤儿文件清理同步）
- 文件层面：参考图复用 asset 文件，没有"删除参考图就删文件"的语义

### Tauri command 层

每实体一份 commands 文件，模式与 `commands/project.rs` 完全一致：
- `create_xxx(state, input) -> Result<Xxx, IpcError>`
- `get_xxx(state, id)`
- `list_xxx(state, opts)`
- `update_xxx(state, id, input)`
- `delete_xxx(state, id) -> Result<(), IpcError>`（所有实体都返回 `()`，Character 无特殊返回）

`lib.rs` 注册（按字母序，显式列出每个函数；tauri-specta 的 `collect_commands!` 宏不支持 `module::*` 通配）：
```rust
.commands(collect_commands![
    commands::character::create_character,
    commands::character::delete_character,
    commands::character::get_character,
    commands::character::list_characters,
    commands::character::update_character,
    commands::costume::create_costume,
    commands::costume::delete_costume,
    commands::costume::get_costume,
    commands::costume::list_costumes,
    commands::costume::update_costume,
    commands::project::create_project,
    commands::project::delete_project,
    commands::project::get_project,
    commands::project::list_projects,
    commands::project::update_project,
    commands::prop::create_prop,
    commands::prop::delete_prop,
    commands::prop::get_prop,
    commands::prop::list_props,
    commands::prop::update_prop,
    commands::scene::create_scene,
    commands::scene::delete_scene,
    commands::scene::get_scene,
    commands::scene::list_scenes,
    commands::scene::update_scene,
])
```

### 前端 hooks

`src/hooks/useCharacters.ts`（与 `useProjects.ts` 同结构）：
```ts
export const characterKeys = {
  all: (projectId: string) => ["characters", projectId] as const,
  detail: (id: string) => ["character", id] as const,
};

export function useCharacterList(projectId: string | undefined) {
  return useQuery<Character[]>({
    queryKey: projectId ? characterKeys.all(projectId) : ["characters", "none"],
    queryFn: () =>
      unwrap(commands.listCharacters({ projectId: projectId as string })),
    enabled: !!projectId,
  });
}

export function useCreateCharacter() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateCharacterInput) =>
      unwrap(commands.createCharacter(input)),
    onSuccess: (_, input) => {
      qc.invalidateQueries({ queryKey: characterKeys.all(input.projectId) });
    },
  });
}

export function useDeleteCharacter(projectId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.deleteCharacter(id)),
    onSuccess: () => {
      // 仅 invalidate 当前 project 的 character + costume 列表
      qc.invalidateQueries({ queryKey: characterKeys.all(projectId) });
      qc.invalidateQueries({ queryKey: ["costumes", projectId] });
    },
  });
}
// ...
```

> **设计说明**：v1 写的 `qc.invalidateQueries({ queryKey: ["characters"] })`（无 projectId 前缀）会误失效所有项目的列表。改成 hook 工厂参数化绑定 projectId，让 invalidation 范围精确。

Scene/Prop/Costume hooks 同模式。Costume 额外提供：
```ts
export function useCostumeListByCharacter(
  projectId: string | undefined,
  characterId: string | undefined,
) {
  return useQuery<Costume[]>({
    queryKey: ["costumes", projectId, "byCharacter", characterId],
    queryFn: () =>
      unwrap(commands.listCostumes({
        projectId: projectId!,
        characterId: characterId!,
      })),
    enabled: !!projectId && !!characterId,
  });
}
```

### CLI（Character）

`crates/cli/src/commands/character.rs`：
```rust
use clap::{Args, Subcommand};
use rusqlite::Connection;

use mango_core::db::queries::character as q;
use mango_core::models::character::{
    CreateCharacterInput, ListCharactersOptions, UpdateCharacterInput,
};

#[derive(Args)]
pub struct CharacterArgs {
    #[command(subcommand)]
    pub command: CharacterCommand,
}

#[derive(Subcommand)]
pub enum CharacterCommand {
    /// List characters under a project
    List {
        #[arg(long)]
        project_id: String,
    },
    /// Create a new character
    Create {
        #[arg(long)]
        project_id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        appearance_prompt: Option<String>,
    },
    /// Show a character's details
    Get { id: String },
    /// Update fields
    Update {
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        appearance_prompt: Option<String>,
    },
    /// Delete a character (cascades to its costumes)
    Delete { id: String },
}

pub fn execute(conn: &Connection, args: CharacterArgs) -> anyhow::Result<()> {
    match args.command {
        CharacterCommand::List { project_id } => {
            let rows = q::list(conn, ListCharactersOptions {
                project_id,
                limit: None,
                offset: None,
            })?;
            for c in rows {
                println!("{}\t{}\t{}", c.id, c.name, c.description);
            }
        }
        CharacterCommand::Create { project_id, name, description, appearance_prompt } => {
            let c = q::create(conn, CreateCharacterInput {
                project_id,
                name,
                description,
                appearance_prompt,
                reference_image_path: None,
            })?;
            println!("{}", c.id);
        }
        // ... Get / Update / Delete 类似
    }
    Ok(())
}
```

`main.rs` 添加：
```rust
enum Commands {
    Project(commands::project::ProjectArgs),
    Character(commands::character::CharacterArgs),     // 新增
    Account(commands::account::AccountArgs),           // 由 spec-13 引入
}
```

> **设计说明**：CLI 的 `Update` 不暴露 `reference_image_path`——拖入文件由 GUI 处理；CLI 想编辑参考图直接 SQL 改也行，不值得做语义保护。

### Scene/Prop/Costume CLI 推迟到 V2

`开发任务.md` 任务 11 仅列了 character + account。本 spec 与之对齐，不实现其他三类的 CLI 命令。在 MS1 验收文档里明确这一点，避免回归疑问。

## 测试策略

### Rust 单元测试（每实体一份）

`db/queries/character.rs` 模块内 `#[cfg(test)] mod tests`：
- `create_then_get` 往返
- `create_empty_name_fails`
- `create_missing_project_fails`（FK 约束触发）
- `list_filters_by_project`（建两个 project，各插入若干角色，验证只返回目标 project 的）
- `list_default_limit_is_50`（插 51 行，验证默认返回 50）
- `update_partial`
- `update_bumps_updated_at`（连续两次 get，updated_at 变大）
- `delete_then_get_returns_not_found`

Costume 额外测试：
- `create_costume_in_wrong_project_fails`
- `delete_character_cascades_costume`（建角色 + 服装 → 删角色 → 验证服装行也消失）

测试 setup 复用 project.rs 的 `setup() -> Connection` 模式（`open_sync(":memory:")`）；但需要先 INSERT 一行 project 再插主体。注意：spec-10 之后 project 表有 root_path，测试 setup 也要带上一个临时路径（用 tempdir + 字符串转换）。

### Tauri command 测试

按既有规则不直接测 command。改用 e2e（实际 IPC 调用）在 MS5 之后 WebdriverIO 接入；MS1 不增加。

### 前端测试

`src/hooks/useCharacters.test.ts`：
- 用 `vi.mock("@/lib/bindings/commands")` mock `commands.listCharacters` 等
- `renderHook(() => useCharacterList("p1"))` 检查 query 状态变迁
- 测 `useDeleteCharacter("p1")` 成功后 invalidate 两个精确 query key（characters/p1, costumes/p1）

## 验收标准

- [ ] `cargo test -p mango-core` 全绿，新增 4 个实体的 CRUD 测试至少 6 用例每实体
- [ ] `Character` / `Scene` / `Prop` / `Costume` 类型导出到 `src/lib/bindings/commands.ts`
- [ ] `pnpm typecheck` 全绿；hooks 文件每个 ≤ 200 行（一文件内不要塞 4 个实体）
- [ ] CLI `mango character list --project-id <id>` / `create` / `get` / `update` / `delete` 全部可用
- [ ] CLI 创建的 character 在 GUI 列表页可见（依赖 spec-14 落地后端到端验收）
- [ ] 删除角色时，其所有服装一并删除（DB CASCADE 触发）；`delete_character` IPC 返回 `()`，前端用 `useCostumeListByCharacter` 在弹窗时单独拉计数
- [ ] 跨项目创建 Costume 被拒绝（Validation 错误）
- [ ] `voice_id` 列在 schema 存在但不出现在 Rust model / TS type 中
- [ ] `updated_at` 在每次 update 后递增（由 `sets.push("updated_at = datetime('now')")` 保证）
- [ ] list 默认 limit 50，最大 200，与 project 对齐
- [ ] `useDeleteCharacter` invalidation 范围限定到 `(projectId, characters)` 与 `(projectId, costumes)`，不影响其他项目

## 依赖关系

- **前置**：spec-10（需要 `project.root_path` 字段已加；Character/Scene/Prop/Costume 实体本身不依赖文件系统，但前端展示参考图需要）；SPEC-04（项目 CRUD 模式参考）
- **后续**：
  - spec-12（素材导入管线）：导入完成后调用 `update_character({ reference_image_path: ... })` 设置参考图
  - spec-14（主体页面）：使用本 spec 暴露的 hooks
  - MS2 任务提交：从 character / scene / prop 中选择实体作为生成 prompt 上下文

## 与原任务文档的差异

`开发任务.md` 把 character/scene/prop/costume 拆成 4 个独立任务。本 spec 合并为一份文档，因四者代码结构 95% 相同，分开会产生重复内容。Costume 的特殊点（FK + 级联）单列小节处理。任务粒度上：
- 实施时建议按 character → scene → prop → costume 顺序提 4 个 PR（每 PR 含 model + queries + commands + hooks + 测试）
- 4 个 PR 共享同一 spec，避免 spec 颗粒度过细
