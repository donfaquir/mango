# General Rules

- Use English for all code, comments, commit messages, and PR descriptions.
- Use Chinese for user-facing UI text and documentation under `docs/`.
- Do not add comments to explain obvious code. Only comment on "why", not "what".
- Prefer editing existing files over creating new ones.
- Do not introduce new dependencies without explicit approval.
- Run `pnpm typecheck` after modifying TypeScript files to verify type safety.
- Run `cargo clippy` after modifying Rust files to catch lint issues.

## Dependency & Version Constraints

**CRITICAL — HIGH PRIORITY PROJECT CONSTRAINT**: This project requires strict use of the latest stable versions. Using outdated frameworks, libraries, toolchains, or language editions is a project-level violation. This applies to ALL version decisions — not just library dependencies, but also Rust edition, Rust toolchain MSRV, TypeScript version, Node.js version, build targets, and any other versioned component.

**NEVER rely on training data for ANY version number.** Training data is always outdated. Before writing or recommending any version:
1. Use `WebFetch` or `WebSearch` to verify the latest stable version from the authoritative source (crates.io, npmjs.com, official docs).
2. Only then compare against the project's pinned versions.

### Before writing any code that involves a dependency:
1. Read `package.json` for frontend dependency versions.
2. Read `Cargo.toml` (workspace root) and the relevant crate's `Cargo.toml` for Rust dependency versions.
3. Read `docs/技术选型.md` for the authoritative tech stack and version requirements.
4. If an iteration spec exists under `docs/iterations/`, read it for feature-specific dependency constraints.

### Version rules:
- For **existing dependencies**: use ONLY the versions pinned in `package.json` / `Cargo.toml`. Do not upgrade or change version ranges without approval.
- For **new dependencies**: if specified in `docs/技术选型.md`, use that version. Otherwise, use `WebSearch` to find the latest stable version and use it. Never fall back to training data.
- When unsure about a library's API at the pinned version, use `WebSearch` or `WebFetch` to check the official docs for that specific version — do not guess.

### When proposing or setting any version (dependency, toolchain, edition, MSRV, build target, etc.):
- **Always verify** the latest stable version via web lookup first.
- **Always prefer** the latest stable version unless there is a concrete, documented compatibility constraint (e.g., tokio-rusqlite 0.7 pins rusqlite ^0.37).
- If a compatibility constraint forces an older version, **document the constraint explicitly** in a comment or the tech doc.
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
