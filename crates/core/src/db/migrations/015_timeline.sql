CREATE TABLE timeline_track (
    id          TEXT PRIMARY KEY NOT NULL,
    episode_id  TEXT NOT NULL REFERENCES episode(id) ON DELETE CASCADE,
    track_type  TEXT NOT NULL CHECK(track_type IN ('video','audio','text','overlay')),
    label       TEXT NOT NULL,
    order_index INTEGER NOT NULL,
    muted       INTEGER NOT NULL DEFAULT 0,
    locked      INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_timeline_track_episode ON timeline_track(episode_id, order_index);

CREATE TABLE timeline_item (
    id            TEXT PRIMARY KEY NOT NULL,
    track_id      TEXT NOT NULL REFERENCES timeline_track(id) ON DELETE CASCADE,
    asset_id      TEXT REFERENCES asset(id),
    item_type     TEXT NOT NULL CHECK(item_type IN ('clip','text','sticker','transition','effect')),
    position_ms   INTEGER NOT NULL,
    duration_ms   INTEGER NOT NULL,
    in_point_ms   INTEGER NOT NULL DEFAULT 0,
    out_point_ms  INTEGER NOT NULL,
    params_json   TEXT NOT NULL DEFAULT '{}',
    order_index   INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_timeline_item_track ON timeline_item(track_id, position_ms);

CREATE TABLE timeline_keyframe (
    id         TEXT PRIMARY KEY NOT NULL,
    item_id    TEXT NOT NULL REFERENCES timeline_item(id) ON DELETE CASCADE,
    property   TEXT NOT NULL CHECK(property IN ('position_x','position_y','scale','rotation','opacity')),
    time_ms    INTEGER NOT NULL,
    value      REAL NOT NULL,
    easing     TEXT NOT NULL DEFAULT 'linear' CHECK(easing IN ('linear','ease_in','ease_out','ease_in_out')),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_timeline_keyframe_item ON timeline_keyframe(item_id, property, time_ms);
