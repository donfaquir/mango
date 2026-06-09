CREATE TABLE shot_audio (
    id          TEXT PRIMARY KEY NOT NULL,
    shot_id     TEXT NOT NULL REFERENCES shot(id) ON DELETE CASCADE,
    asset_id    TEXT NOT NULL REFERENCES asset(id) ON DELETE CASCADE,
    audio_role  TEXT NOT NULL DEFAULT 'sfx' CHECK(audio_role IN ('voice','sfx','bgm')),
    volume      REAL NOT NULL DEFAULT 1.0 CHECK(volume >= 0.0 AND volume <= 2.0),
    offset_ms   INTEGER NOT NULL DEFAULT 0,
    order_index INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_shot_audio_shot ON shot_audio(shot_id, audio_role, order_index);
CREATE INDEX idx_shot_audio_asset ON shot_audio(asset_id);
