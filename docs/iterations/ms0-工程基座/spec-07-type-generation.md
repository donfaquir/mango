# SPEC-07: tauri-specta + ts-rs 类型生成管线

## 概述

Rust → TypeScript 类型生成的目标是：**前后端共用一份类型，由 Rust 单边声明，构建期自动产出 TS**。

本项目采用 **单源策略**：

- **凡是被 Tauri command 引用的类型**（参数、返回值、错误）一律走 **tauri-specta + specta**，与 command 同时生成到 `src/lib/bindings/commands.ts`。
- **ts-rs 保留但当前未使用**，预留给后续场景：Tauri Event payload、与 command 解耦的纯数据 enum、CLI 输出 schema。这类类型 specta 不会跟着 command 一起导出，需要独立的导出通道。

> 历史背景：早期方案是 specta 和 ts-rs 双源、按命名拆分。实施过程中发现，只要类型出现在 command 签名上，specta 一定会重新发现并导出一份；让 ts-rs 同时导出会得到两份**同名但路径不同**的 TS 类型，前端 import 时容易踩到错的那个。所以收敛为单源。

## 技术方案

### 目录/文件结构

```
src/lib/bindings/
├── commands.ts             # tauri-specta 生成 — 唯一活跃出口
└── types/                  # 留作 ts-rs 输出 event/独立类型（当前为空）

src-tauri/src/
└── lib.rs                  # specta Builder 配置（见 SPEC-05）

crates/core/src/models/
└── project.rs              # 业务类型；当前全部 #[derive(specta::Type)]
```

### 依赖项

已在 SPEC-01、SPEC-04、SPEC-05 中固定（workspace 根 `Cargo.toml`）：

```toml
specta            = { version = "2.0.0-rc.25", features = ["derive"] }
specta-typescript = "0.0.12"
ts-rs             = { version = "12", features = ["chrono-impl", "uuid-impl"] }
```

`src-tauri/Cargo.toml` 额外加 `tauri-specta = "2.0.0-rc.25"` (`features = ["typescript", "derive"]`)。

`crates/core/Cargo.toml` 引用了 `specta`、`specta-typescript`、`ts-rs` 三者，但只有 specta 在实际生效；保留 `specta-typescript` 是因为 i64/u64/usize 的 `#[specta(type = Number)]` 类型覆写需要它的 `Number` marker（详见下文）。

### 核心实现

#### tauri-specta（command-bound 类型）

**Builder 在 `src-tauri/src/lib.rs`：**

```rust
let builder = Builder::<tauri::Wry>::new()
    .commands(collect_commands![
        commands::project::create_project,
        commands::project::get_project,
        commands::project::list_projects,
        commands::project::update_project,
        commands::project::delete_project,
    ]);

#[cfg(debug_assertions)]
builder
    .export(Typescript::default(), "../src/lib/bindings/commands.ts")
    .expect("Failed to export tauri-specta typescript bindings");
```

**类型声明放在 `crates/core/src/models/`，命令函数放在 `src-tauri/src/commands/`：**

```rust
// crates/core/src/models/project.rs
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Project {
    pub id: String,
    pub name: String,
    // i64 默认会触发 specta 的 BigInt-forbidden 保护（避免 JS 精度丢失）。
    // 业务上这些值远小于 2^53，覆写为 TS `number`。
    #[specta(type = Option<specta_typescript::Number>)]
    pub global_seed: Option<i64>,
    // ...
}
```

```rust
// src-tauri/src/commands/project.rs
#[tauri::command]
#[specta::specta]
pub async fn create_project(
    state: State<'_, AppState>,
    input: CreateProjectInput,
) -> Result<Project, IpcError> {
    with_db(&state, move |conn| project_queries::create(conn, input)).await
}
```

`IpcError`（`src-tauri/src/error.rs`）也 `#[derive(specta::Type)]`，确保错误形状一并出现在 `commands.ts`。

**生成时机**：

- 仅在 `cfg(debug_assertions)` 下生效；release 构建不重写。
- `pnpm tauri dev` 启动 Rust 二进制时，`run()` 在 Tauri Builder 起来之前就同步执行 export，文件落到 `src/lib/bindings/commands.ts`。
- 单纯 `cargo build -p mango-tauri` 不会触发 export（没有运行 `run()`）。
- 也可以临时手动跑：起一次 `pnpm tauri dev`，等到 `commands.ts` 出现就关掉。

#### ts-rs（事件/独立类型，当前未使用）

ts-rs 留给两类场景，**两类都不在 MS0 范围内**：

1. **Tauri Event payload**（`app.emit("...", payload)` 的 payload 形状）。这种类型不出现在 command 签名上，specta 不会扫到。
2. **不被任何 command 引用的纯数据类型**。例如某些只在 frontend 间共享的 enum schema。

未来真正用上时，约定的写法（仅作示例，不要现在落地）：

```rust
// 假设的 future event payload
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/types/")]
pub struct TaskProgressEvent {
    pub task_id: String,
    pub progress: f32,
}
```

ts-rs 通过 `#[ts(export)]` 在 `cargo test` 时生成 `.ts` 文件。配套的触发测试：

```rust
// 仅当 ts-rs 类型存在时才需要这个测试
#[cfg(test)]
mod export_bindings {
    #[test]
    fn export_ts_types() {
        // ts-rs 在 #[ts(export)] 上自动注册的导出会在 cargo test 内执行
    }
}
```

> 单源原则：**同一个类型不要同时挂 `Type` 和 `TS` 两个 derive。** specta 已发现的，ts-rs 不再产；ts-rs 导出的，不要被 command 引用。

#### i64 / u64 的特殊处理

specta-typescript 默认禁止把 `i64` / `u64` / `usize` / `isize` / `i128` / `u128` 直接导出 —— TS `number` 在 2^53 以上会丢精度。两种解决方式：

| 选项 | 用法 | 何时选 |
|---|---|---|
| `#[specta(type = specta_typescript::Number)]` | 字段级覆写为 TS `number` | 业务上保证不超 2^53（分页 limit/offset、用户提供的 RNG seed） |
| `#[specta(type = specta_typescript::BigInt)]` | 字段级覆写为 TS `bigint` | 真实可能超过 2^53（文件大小、纳秒时间戳） |

当前 `Project.global_seed`、`ListProjectsOptions.{limit, offset}` 都用 `Number` 覆写。

#### 前端消费模式

生成的 `commands.ts` 形状（节选，由 tauri-specta 写入，不要手改）：

```typescript
export const commands = {
  createProject: (input: CreateProjectInput) =>
    typedError<Project, IpcError>(__TAURI_INVOKE("create_project", { input })),
  listProjects: (opts: ListProjectsOptions | null) =>
    typedError<Project[], IpcError>(__TAURI_INVOKE("list_projects", { opts })),
  // ...
};

export type Project = { id: string; name: string; /* ... */ };
export type IpcError = { message: string; code: string };
```

`typedError<T, E>` 把调用结果包成 `{ status: "ok"; data: T } | { status: "error"; error: E }`。前端不要直接用，统一过 `src/lib/ipc.ts` 的 `unwrap()` 转成"成功值/抛异常"以便 TanStack Query 直接接：

```typescript
// src/hooks/useProjects.ts
import { commands, type CreateProjectInput } from "@/lib/bindings/commands";
import { unwrap } from "@/lib/ipc";

export function useProjectList() {
  return useQuery({
    queryKey: ["projects"],
    queryFn: () => unwrap(commands.listProjects(null)),
  });
}

export function useCreateProject() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateProjectInput) =>
      unwrap(commands.createProject(input)),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["projects"] }),
  });
}
```

`unwrap()` 失败时抛出 `IpcCallError`（带 `code` 字段，便于按 `NOT_FOUND` / `VALIDATION_ERROR` / `DB_ERROR` 分支处理）。

### .gitignore 策略

`src/lib/bindings/commands.ts` **提交进仓**。理由：

- code review 能直接看到 IPC 接口形状的 diff，比"接口契约只活在 dev 启动产物"安全。
- 新开发者 clone 后 `pnpm typecheck` 立即通过，不需要先跑一次 Rust 才能让 TS 编译。
- 文件是确定性输出，diff 不会噪声。

代价：每次改 command 签名都会有 `commands.ts` 的额外 diff。这是预期的，由作者负责一并提交。

### 开发流程

```
开发者改 Rust command 签名 / specta 类型
        ↓
  pnpm tauri dev（Rust 重新编译）
        ↓
  builder.export() 重写 src/lib/bindings/commands.ts
        ↓
  Vite HMR 拾取新文件
        ↓
  TypeScript 用法不匹配 → tsc 报错（IDE 红线 + pnpm typecheck 失败）
        ↓
  作者一并提交 .rs 和 commands.ts 两边的 diff
```

未来引入 ts-rs 类型时，工作流多一步：`cargo test -p mango-core` 触发 `types/*.ts` 重新生成。

### package.json 便捷脚本（可选）

当前未引入。等 ts-rs 真正承担生成时再加：

```json
{
  "scripts": {
    "gen:types": "cargo test -p mango-core --quiet -- --nocapture export_ts_types"
  }
}
```

## 接口定义

实际产出（截至 MS0 完成）：

```typescript
// src/lib/bindings/commands.ts （由 tauri-specta 自动生成）
import { invoke as __TAURI_INVOKE } from "@tauri-apps/api/core";

export const commands = {
  createProject: (input: CreateProjectInput) => /* ... */,
  getProject: (id: string) => /* ... */,
  listProjects: (opts: ListProjectsOptions | null) => /* ... */,
  updateProject: (id: string, input: UpdateProjectInput_Deserialize) => /* ... */,
  deleteProject: (id: string) => /* ... */,
};

export type Project = {
  id: string;
  name: string;
  description: string;
  style_prompt: string;
  global_seed: number | null;
  created_at: string;
  updated_at: string;
};

export type CreateProjectInput = {
  name: string;
  description?: string | null;
  style_prompt?: string | null;
  global_seed?: number | null;
};

export type ListProjectsOptions = {
  limit?: number | null;
  offset?: number | null;
};

export type IpcError = {
  message: string;
  code: string;
};

// UpdateProjectInput 拆成 _Serialize / _Deserialize 两个变体，
// 因为 Rust 端用 Option<Option<i64>> 表达"不修改/置空/赋值"三态，
// 经 JSON 序列化后只剩 number | null —— TS 类型层面塌缩为同一形状，
// 三态区分只能在运行时通过"字段缺失 vs null vs 数字"约定实现。
```

## 验收标准

```bash
# 1. 启动一次 dev 触发 export
pnpm tauri dev   # 待窗口出现后即可关闭
# → src/lib/bindings/commands.ts 存在

# 2. TS 编译通过
pnpm typecheck   # tsc --noEmit

# 3. 前端可正常 import 并消费
#    在 hooks/useProjects.ts 中已用 commands.* + unwrap() 模式

# 4. 类型安全：故意把 createProject 的 name 传成 number
#    → tsc 报错（已通过 hooks/useProjects.ts 写法验证）

# 5. 修改 Rust 字段（验证回路）
#    给 Project 加一个 String 字段 → pnpm tauri dev → commands.ts 自动出现新字段
#    → 旧的前端代码立即在 tsc 中获得类型提示
```

## 与早期文档的差异

| 项 | 旧 spec | 现状 |
|---|---|---|
| 单一来源 | specta + ts-rs 并行 | **specta 单源**，ts-rs 仅留给未来 event |
| `types/*.ts` | 由 ts-rs 生成 | 当前空目录，作为 ts-rs 占位 |
| 前端 import | `import { listProjects } from ".../commands"` | `import { commands } from ".../commands"` + `commands.listProjects(...)` |
| 错误类型 | 直接 `Promise<T>` | tauri-specta Result 元组，`unwrap()` 解开 |
| `IpcError` | 未在 commands.ts 中 | derives `specta::Type`，自动出现在 `commands.ts` |
| i64 处理 | 未提及 | 必须 `#[specta(type = Number)]` 或 `BigInt` 覆写 |
| specta-typescript 版本 | 0.0.8 | 0.0.12 |

## 依赖关系

- **前置**：SPEC-01 / SPEC-02 / SPEC-04 / SPEC-05
- **后续**：SPEC-08 / SPEC-09 已基于此管线消费 `commands.ts`；MS1+ 新 command 只需加进 `collect_commands!`，新类型加 `#[derive(specta::Type)]` 即可自动入流。
