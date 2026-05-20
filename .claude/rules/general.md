# General Rules

- Use English for all code, comments, commit messages, and PR descriptions.
- Use Chinese for user-facing UI text and documentation under `docs/`.
- Do not add comments to explain obvious code. Only comment on "why", not "what".
- Prefer editing existing files over creating new ones.
- Do not introduce new dependencies without explicit approval.
- Run `pnpm typecheck` after modifying TypeScript files to verify type safety.
- Run `cargo clippy` after modifying Rust files to catch lint issues.

## Dependency Version Constraints

**CRITICAL**: Never rely on training data or memory for dependency versions, APIs, or usage patterns. Always read the project's source of truth before writing code.

Before writing any code that involves a dependency:
1. Read `package.json` for frontend dependency versions.
2. Read `Cargo.toml` (workspace root) and the relevant crate's `Cargo.toml` for Rust dependency versions.
3. Read `docs/技术选型.md` for the authoritative tech stack and version requirements.
4. If an iteration spec exists under `docs/iterations/`, read it for feature-specific dependency constraints.

Rules:
- For **existing dependencies**: use ONLY the versions pinned in `package.json` / `Cargo.toml`. Do not upgrade or change version ranges without approval.
- For **new dependencies**: if specified in `docs/技术选型.md`, use that version. Otherwise, use `WebSearch` to find the latest stable version and use it. Never fall back to training data — versions from training are likely outdated.
- When unsure about a library's API at the pinned version, use `WebSearch` or `WebFetch` to check the official docs for that specific version — do not guess.
- Tailwind CSS 4 has breaking changes from v3 (CSS-first config, no `tailwind.config.ts`). Always verify usage against v4 docs.
- shadcn/ui, React 19, Zustand 5, TanStack Query 5, React Router 7, Tauri 2 all have major API differences from prior versions. When in doubt, verify before writing code.

## File Size & Code Organization

**Single files must not grow large.** Split code by business domain / feature / responsibility.

### Size Limits

- **TypeScript/TSX**: max ~200 lines per file. Exceeding 150 lines should trigger a split consideration.
- **Rust**: max ~300 lines per file. Exceeding 250 lines should trigger a split consideration.
- If you are about to write code that would push a file beyond these limits, **stop and split first**.

### How to Split

**React frontend:**
- One component per file. Extract sub-components into separate files when they have independent logic.
- Custom hooks → `hooks/useXxx.ts`
- Utility / helper functions → `lib/` or colocated `utils.ts`
- Type definitions shared across files → colocated `types.ts`
- Constants / config → colocated `constants.ts`
- Organize by feature under components: `components/<feature_name>/`, with sub-files for each component, hooks, types, etc.

**Rust backend:**
- One module = one file or one directory with `mod.rs` + sub-files.
- Split by responsibility: types in `types.rs`, errors in `error.rs`, business logic in domain-named files.
- Public API surface in `mod.rs` — re-export, don't implement.
- Query functions per entity: `db/queries/project.rs`, `db/queries/shot.rs`, etc.

### Directory Structure Discipline

- When a directory has 6+ files, consider grouping into subdirectories by sub-domain.
- Every new directory must have a clear single responsibility. Name it after what it contains, not what it does (nouns > verbs).
- Before creating a new file, check if an existing file in the same domain is the right place. Before creating a new directory, check if an existing directory already covers that domain.
