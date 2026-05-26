-- spec-16: arbitrary per-account JSON parameters. Today only `oss` (sub-object
-- holding endpoint/bucket/access_key_id/region/url_expires_seconds) is read.
-- The access_key_secret stays in keyring entry api_account:<id>:oss_secret.
-- NOT NULL DEFAULT '{}' is the only ALTER form SQLite allows for a non-null
-- column; older rows transparently get a valid empty JSON object.
ALTER TABLE api_account ADD COLUMN params_json TEXT NOT NULL DEFAULT '{}';
