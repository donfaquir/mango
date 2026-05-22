# CLAUDE.md — Mango Project

## Project Overview

Mango is a **local-first desktop app** for AI comic/manga video creation — from script to final cut. Built with **Tauri 2 (Rust backend) + React 19 (frontend)**. Targets individual creators and small studios (2-5 people).

## Architecture

```
React (WebView) ──Tauri IPC──▸ src-tauri ──▸ crates/core (business logic)
                                            crates/cli  (CLI binary)
```

- **crates/core**: Pure Rust library, no Tauri dependency. Contains DB, FFmpeg, task engine, provider abstraction, asset management, account management, checkpoint/backup.
- **crates/cli**: CLI binary using clap, consumes core directly.
- **src-tauri**: Thin Tauri command layer wrapping core functions. Handles IPC serialization and Tauri state injection only.
- **src/**: React frontend with TypeScript strict mode.

## Tech Stack

### Rust Backend
- **Async runtime**: tokio 1.x
- **Database**: rusqlite + tokio-rusqlite (SQLite, async wrapper)
- **Serialization**: serde + serde_json
- **Error handling**: thiserror (libraries) + anyhow (binaries)
- **Logging**: tracing + tracing-subscriber
- **Type export**: tauri-specta (command bindings) + ts-rs (pure struct export to `src/lib/bindings/`)

### Frontend
- **Framework**: React 19 + TypeScript 6.x (strict mode)
- **Build**: Vite 8+ with @vitejs/plugin-react
- **Styling**: Tailwind CSS 4 (CSS-first config, via @tailwindcss/vite plugin)
- **UI Components**: shadcn/ui (new-york style) + Radix Primitives + lucide-react icons
- **State**: Zustand 5 (UI state only) + TanStack Query 5 (server/IPC data)
- **Routing**: react-router-dom 7
- **Canvas**: @xyflow/react (React Flow) for node-based editing
- **Testing**: Vitest + @testing-library/react + jsdom

### Path Aliases
- `@/*` maps to `./src/*` (configured in tsconfig.json and vite.config.ts)
- shadcn/ui components: `@/components/ui`
- Utilities: `@/lib/utils`
- Hooks: `@/hooks`

## Common Commands

```bash
# Frontend
pnpm dev              # Start Vite dev server (port 5173)
pnpm build            # TypeScript check + Vite build
pnpm test             # Vitest in watch mode
pnpm test:run         # Vitest single run
pnpm typecheck        # TypeScript type check only (tsc --noEmit)

# Tauri
pnpm tauri dev        # Launch Tauri dev mode (Rust + frontend)
pnpm tauri build      # Production build (DMG/EXE/AppImage)

# Rust
cargo build           # Build all workspace crates
cargo test            # Run all Rust tests
cargo test -p mango-core  # Test core crate only
cargo clippy          # Lint Rust code
```

## Project Structure

```
src/                    # React frontend
├── components/ui/      # shadcn/ui components
├── hooks/              # Custom React hooks
├── lib/                # Utilities and type bindings
│   ├── bindings/       # Auto-generated TS types from Rust (ts-rs)
│   └── utils.ts        # cn() helper (clsx + tailwind-merge)
├── pages/              # Route pages
├── stores/             # Zustand stores
├── app.css             # Global styles + Tailwind
├── App.tsx             # Root component
└── main.tsx            # Entry point

crates/
├── core/src/           # Core business logic (pure Rust)
│   ├── db/             # Schema, migrations, queries
│   ├── ffmpeg/         # FFmpeg command wrapper
│   ├── task_engine/    # Async task scheduler
│   ├── provider/       # AI model provider abstraction (ModelProvider trait)
│   ├── asset/          # File management, thumbnails, archiving
│   ├── account/        # API key storage, usage tracking
│   └── checkpoint/     # Version snapshots, SQLite backup
└── cli/src/            # CLI binary (clap)

src-tauri/src/          # Tauri app shell
├── main.rs             # Tauri entry
└── lib.rs              # Command registration
```

## Coding Conventions

### Rust
- Use `thiserror` for error types in core crate, `anyhow` in CLI/Tauri binaries
- All Tauri commands return `Result<T, String>`
- Core crate must not depend on Tauri APIs — keep it a pure library
- Use `tracing` macros for logging (not println!)
- Derive `serde::Serialize` + `serde::Deserialize` for all IPC types
- Use `#[derive(TS)]` from ts-rs for types that need frontend bindings

### TypeScript / React
- Strict TypeScript — no `any`, no `@ts-ignore`
- Use `cn()` from `@/lib/utils` for conditional class merging (clsx + tailwind-merge)
- State separation: Zustand for UI state, TanStack Query for server/IPC data
- Use `@/` path alias for all imports within src/
- Component files use PascalCase (e.g., `ProjectList.tsx`)
- Hooks use camelCase with `use` prefix (e.g., `useProject.ts`)
- Store files use camelCase (e.g., `canvasStore.ts`)

### Testing
- Frontend tests go in `src/test/` or colocated `*.test.tsx` files
- Vitest setup file: `src/test/setup.ts`
- Rust tests: inline `#[cfg(test)]` modules or `tests/` directory in each crate

### Git
- Branch naming: `feature/YYYYMMDD-description` (e.g., `feature/20260520-code-init-1`)
- Write commit messages in English
- Keep PRs focused — one feature or fix per PR

## Worklog 工作日志习惯

每个 Claude session 必须维护 `docs/worklog/YYYY-MM-DD.md`，让用户次日打开就能续接工作。

### Session 启动时（第一次回应用户之前）

必读最近一份 worklog 获取上下文：
1. 先读 `docs/worklog/<今天>.md`；如不存在或为空，读 `ls -1t docs/worklog/*.md | head -2` 找最近 1-2 天的文件
2. 重点看最近 session 段落里的「下一步」字段，作为接续点
3. SessionStart hook 已自动在当日 worklog 写入本 session 的段落头（`## 时间 · session:xxx · branch:xxx`），无需手写

### 工作进行中（关键节点主动 append）

以下时机必须在当前 session 段落下 append 一段记录到当日 worklog：
- 一个 `TaskCreate` 任务被标记 completed 时
- 做出重要架构/技术决策时
- 遇到无法自行解决的阻塞（需要用户决策、外部依赖）时
- Session 即将结束、要交接给用户时

### Append 格式

```markdown
### HH:MM <一行总结，10 字内>
- **做了什么**: 简述动作和涉及的文件
- **为什么**: 动机、约束、上下文（用户次日看时能复原决策）
- **下一步**: 明确的接续动作，让下个 session 知道从哪里开始
```

「下一步」是核心字段——次日新 session 启动后会优先读它来决定接什么。

## Key Design Decisions

- **Local-first**: Data stays on user's machine. Network only for external AI model API calls.
- **Model-agnostic**: Any HTTP API model provider can be integrated via the `ModelProvider` trait.
- **IPC type safety**: tauri-specta auto-generates TS bindings from Rust commands. ts-rs exports standalone types to `src/lib/bindings/`.
- **Canvas persistence**: React Flow state serialized to JSON, stored in DB per episode, auto-saved with 1s debounce.
- **Asset management**: SQLite stores metadata only. Files stored in project directory with asset_id as filename, relative paths throughout.
