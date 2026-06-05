CREATE TABLE video_clip (
    id              TEXT PRIMARY KEY NOT NULL,
    project_id      TEXT NOT NULL REFERENCES project(id),
    episode_id      TEXT REFERENCES episode(id),
    source_asset_id TEXT NOT NULL REFERENCES asset(id),
    label           TEXT,
    trim_start_ms   INTEGER,
    trim_end_ms     INTEGER,
    order_index     INTEGER NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_video_clip_episode ON video_clip(episode_id, order_index);
