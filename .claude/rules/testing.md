---
paths:
  - "src/**/*.test.ts"
  - "src/**/*.test.tsx"
  - "src/test/**/*"
  - "crates/*/src/**/*.rs"
  - "crates/*/tests/**/*.rs"
---

# Testing Rules

## Frontend (Vitest + Testing Library)

- Test files colocated next to source: `Component.test.tsx` alongside `Component.tsx`, or under `src/test/`.
- Use `@testing-library/react` — query by role, label, or text. Avoid `getByTestId` unless no semantic alternative.
- Mock Tauri IPC calls in tests. Do not call real backend from frontend tests.
- Run `pnpm test:run` to verify all tests pass before committing.

## Rust (cargo test)

- Use inline `#[cfg(test)] mod tests {}` for unit tests within each module.
- Use `tests/` directory at the crate root for integration tests.
- Test core crate logic directly. Do not test through Tauri command wrappers.
- Use in-memory SQLite (`:memory:`) for DB tests. Do not leave test database files on disk.
- Run `cargo test -p mango-core` to test core logic in isolation.
