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

## Behavioral Guidelines

> Source: [multica-ai/andrej-karpathy-skills · CLAUDE.md](https://github.com/multica-ai/andrej-karpathy-skills/blob/main/CLAUDE.md). High-level behavior rules; the project-specific conventions below are concrete applications of these.

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

### 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them — don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

### 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

### 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it — don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: every changed line should trace directly to the user's request.

### 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:

```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

**These guidelines are working if:** fewer unnecessary changes in diffs, fewer rewrites due to overcomplication, and clarifying questions come before implementation rather than after mistakes.

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
