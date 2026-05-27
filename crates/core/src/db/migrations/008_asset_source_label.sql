-- Asset provenance + user-defined label.
--
-- `source` distinguishes user-imported bytes from runner-generated artifacts
-- so the asset library UI can filter the two flows independently. The CHECK
-- constraint is attached at column-add time (SQLite supports this in
-- ALTER TABLE ADD COLUMN as long as the constraint is column-scoped).
--
-- `label` is a free-form classification tag set by the user (defaults to ''
-- for backwards compatibility with rows created before this migration).
--
-- Backfill rule: any asset already pointed at by a generation_task's
-- result_asset_id is mechanically a generated artifact, regardless of when
-- it was created — flip those rows to 'generated' so the new filter
-- behaves consistently for existing projects.
ALTER TABLE asset ADD COLUMN source TEXT NOT NULL DEFAULT 'imported'
    CHECK(source IN ('imported', 'generated'));

ALTER TABLE asset ADD COLUMN label TEXT NOT NULL DEFAULT '';

UPDATE asset SET source = 'generated'
WHERE id IN (
    SELECT result_asset_id FROM generation_task WHERE result_asset_id IS NOT NULL
);

CREATE INDEX idx_asset_source ON asset(project_id, source);
CREATE INDEX idx_asset_type_source ON asset(project_id, asset_type, source);
