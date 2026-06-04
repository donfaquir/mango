ALTER TABLE shot ADD COLUMN adopted_asset_id TEXT REFERENCES asset(id) ON DELETE SET NULL;
CREATE INDEX idx_shot_adopted_asset ON shot(adopted_asset_id);
