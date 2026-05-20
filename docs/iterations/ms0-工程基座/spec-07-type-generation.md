# SPEC-07: tauri-specta + ts-rs 类型生成管线

## 概述

配置完整的 Rust → TypeScript 类型生成管线。tauri-specta 自动从 Tauri commands 生成带类型的调用函数；ts-rs 用于非 command 场景（如 event payload、独立 enum）。两者配合确保前后端类型永远一致。

## 技术方案

### 目录/文件结构

```
src/lib/bindings/
├── commands.ts             # tauri-specta 生成 — command 函数绑定
└── types/                  # ts-rs 生成 — 纯数据类型
    ├── Project.ts
    ├── CreateProjectInput.ts
    └── UpdateProjectInput.ts

src-tauri/src/
└── lib.rs                  # specta Builder 配置（已在 SPEC-05 中建立）
```

### 依赖项

已在 SPEC-01 和 SPEC-05 中配置：
- `tauri-specta = "2.0.0-rc"` (features: typescript, derive)
- `specta = "2.0.0-rc"` (features: typescript)
- `specta-typescript = "0.0.8"`
- `ts-rs = "12"` (在 core crate 中)

### 核心实现

#### tauri-specta 配置（已在 SPEC-05 lib.rs 中）

关键点回顾：
```rust
let builder = Builder::<tauri::Wry>::new()
    .commands(collect_commands![...]);

#[cfg(debug_assertions)]
builder
    .export(Typescript::default(), "../src/lib/bindings/commands.ts")
    .expect("Failed to export typescript bindings");
```

**生成时机**：
- 开发模式（`pnpm tauri dev`）启动时自动生成
- 仅在 `debug_assertions` 下执行，生产构建不触发
- 每次 Tauri 进程启动都会重新生成，确保最新

#### ts-rs 配置（core crate 中的 struct）

```rust
// crates/core/src/models/project.rs
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/types/")]
pub struct Project { ... }
```

**生成时机**：
- 通过 `cargo test` 触发（ts-rs 的 export 在 test 中执行）
- 可添加专用 test：

```rust
// crates/core/src/lib.rs
#[cfg(test)]
mod tests {
    #[test]
    fn export_ts_bindings() {
        // ts-rs 的 #[ts(export)] 会在 cargo test 时自动生成文件
        // 此测试确保类型能正确导出
    }
}
```

#### 前端消费模式

**直接使用 commands.ts 中的函数：**
```typescript
// src/hooks/useProjects.ts
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { listProjects, createProject, deleteProject } from "@/lib/bindings/commands";
import type { CreateProjectInput } from "@/lib/bindings/types/CreateProjectInput";

export function useProjectList() {
  return useQuery({
    queryKey: ["projects"],
    queryFn: () => listProjects(),
  });
}

export function useCreateProject() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateProjectInput) => createProject(input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["projects"] });
    },
  });
}

export function useDeleteProject() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => deleteProject(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["projects"] });
    },
  });
}
```

#### .gitignore 配置

bindings 目录是自动生成的，应加入 .gitignore：
```
# 自动生成的类型绑定
src/lib/bindings/commands.ts
src/lib/bindings/types/
```

但为了 CI/CD 和新开发者体验，也可选择提交。**建议提交**（便于 code review 发现接口变更）。

#### 开发流程

```
开发者修改 Rust command/struct
        ↓
    pnpm tauri dev（重启）
        ↓
  tauri-specta 重新生成 commands.ts
        ↓
  cargo test 重新生成 types/*.ts
        ↓
  前端 import 自动获得新类型
        ↓
  TypeScript 编译器报错如果用法不匹配
```

### 配置

**package.json 添加便捷脚本：**
```json
{
  "scripts": {
    "gen:types": "cd src-tauri && cargo test export_ts_bindings",
    "typecheck": "tsc --noEmit"
  }
}
```

## 接口定义

**生成的 commands.ts 示例结构：**
```typescript
// 此文件由 tauri-specta 自动生成，请勿手动修改
import { invoke } from "@tauri-apps/api/core";

// Types
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
  description: string | null;
  style_prompt: string | null;
  global_seed: number | null;
};

export type UpdateProjectInput = {
  name: string | null;
  description: string | null;
  style_prompt: string | null;
  global_seed: number | null;
};

export type IpcError = {
  message: string;
  code: string;
};

// Commands
export async function createProject(input: CreateProjectInput): Promise<Project> {
  return await invoke("create_project", { input });
}

export async function getProject(id: string): Promise<Project> {
  return await invoke("get_project", { id });
}

export async function listProjects(): Promise<Project[]> {
  return await invoke("list_projects");
}

export async function updateProject(id: string, input: UpdateProjectInput): Promise<Project> {
  return await invoke("update_project", { id, input });
}

export async function deleteProject(id: string): Promise<void> {
  return await invoke("delete_project", { id });
}
```

## 验收标准

```bash
# 1. tauri-specta 生成验证
pnpm tauri dev
# → src/lib/bindings/commands.ts 存在且内容正确

# 2. ts-rs 生成验证
cargo test -p mango-core
# → src/lib/bindings/types/ 下生成 .ts 文件

# 3. TypeScript 编译无错误
npx tsc --noEmit

# 4. 生成的函数可在前端调用
# 在 React 组件中 import 并调用，确认类型提示完整

# 5. 修改 Rust struct 后验证更新
# 给 Project 加一个字段 → cargo test → 重启 tauri dev
# → TypeScript 中使用新字段有类型提示

# 6. 类型安全验证
# 故意传错参数类型 → TypeScript 编译报错
```

## 依赖关系

- **前置**：
  - SPEC-01（workspace 和依赖配置）
  - SPEC-02（前端工程 + tsconfig）
  - SPEC-04（Project struct 含 ts-rs derive）
  - SPEC-05（tauri-specta Builder 配置）
- **后续**：
  - SPEC-08 和 SPEC-09 依赖此管线生成的绑定
  - 后续所有新增 command 只需加入 collect_commands! 即可自动生成
