-- ============================================================
-- Mango Schema V2 — add project.root_path
-- ============================================================
-- The column is created NULL-able because SQLite ALTER TABLE ADD COLUMN
-- cannot add a NOT NULL column without a static default, and the correct
-- default depends on per-installation app_data_dir. The application layer
-- backfills any NULL rows at startup (see crates/core/src/startup.rs) and
-- never writes NULL after MS1.

ALTER TABLE project ADD COLUMN root_path TEXT;
