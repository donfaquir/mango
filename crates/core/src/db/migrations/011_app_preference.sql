CREATE TABLE app_preference (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL
);

INSERT INTO app_preference (key, value_json) VALUES ('task.max_concurrency', '3');
