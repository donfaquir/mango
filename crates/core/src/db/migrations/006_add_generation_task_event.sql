CREATE TABLE generation_task_event (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id      TEXT NOT NULL REFERENCES generation_task(id) ON DELETE CASCADE,
    occurred_at  TEXT NOT NULL DEFAULT (datetime('now')),
    phase        TEXT NOT NULL CHECK(phase IN
                  ('submit_upload','submit_call','poll','download','persist','cleanup')),
    severity     TEXT NOT NULL CHECK(severity IN ('info','warn','error')),
    request_id   TEXT,
    http_status  INTEGER,
    details_json TEXT NOT NULL DEFAULT '{}',
    message      TEXT NOT NULL
);
CREATE INDEX idx_generation_task_event_task ON generation_task_event(task_id, occurred_at);
