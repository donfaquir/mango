-- ============================================================
-- Mango Schema V1
-- ============================================================

-- 项目
CREATE TABLE project (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    style_prompt TEXT NOT NULL DEFAULT '',
    global_seed INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 集/章节
CREATE TABLE episode (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    order_index INTEGER NOT NULL DEFAULT 0,
    script_text TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_episode_project ON episode(project_id);

-- 分镜
CREATE TABLE shot (
    id TEXT PRIMARY KEY,
    episode_id TEXT NOT NULL REFERENCES episode(id) ON DELETE CASCADE,
    order_index INTEGER NOT NULL DEFAULT 0,
    summary TEXT NOT NULL DEFAULT '',
    duration_sec REAL,
    camera_angle TEXT NOT NULL DEFAULT '',
    shot_type TEXT NOT NULL DEFAULT '',
    mood TEXT NOT NULL DEFAULT '',
    dialogue TEXT NOT NULL DEFAULT '',
    video_prompt TEXT NOT NULL DEFAULT '',
    image_prompt TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','ready','generating','done')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_shot_episode ON shot(episode_id);

-- 角色
CREATE TABLE character_profile (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    appearance_prompt TEXT NOT NULL DEFAULT '',
    reference_image_path TEXT,
    voice_id TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_character_project ON character_profile(project_id);

-- 场景
CREATE TABLE scene (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    environment_prompt TEXT NOT NULL DEFAULT '',
    reference_image_path TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_scene_project ON scene(project_id);

-- 道具
CREATE TABLE prop (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    reference_image_path TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_prop_project ON prop(project_id);

-- 服装
CREATE TABLE costume (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    character_id TEXT NOT NULL REFERENCES character_profile(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    reference_image_path TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_costume_project ON costume(project_id);
CREATE INDEX idx_costume_character ON costume(character_id);

-- 素材文件
CREATE TABLE asset (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    shot_id TEXT REFERENCES shot(id) ON DELETE SET NULL,
    asset_type TEXT NOT NULL CHECK(asset_type IN ('image','video','audio','script')),
    original_name TEXT NOT NULL,
    file_path TEXT NOT NULL,
    thumbnail_path TEXT,
    file_size INTEGER NOT NULL DEFAULT 0,
    content_hash TEXT,
    metadata_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_asset_project ON asset(project_id);
CREATE INDEX idx_asset_shot ON asset(shot_id);

-- 模型提供商
CREATE TABLE provider (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    base_url TEXT NOT NULL DEFAULT '',
    auth_type TEXT NOT NULL DEFAULT 'api_key' CHECK(auth_type IN ('api_key','oauth')),
    docs_url TEXT NOT NULL DEFAULT ''
);

-- 模型
CREATE TABLE model (
    id TEXT PRIMARY KEY,
    provider_id TEXT NOT NULL REFERENCES provider(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    model_type TEXT NOT NULL CHECK(model_type IN ('text','image','video','audio')),
    capabilities_json TEXT,
    default_params_json TEXT
);
CREATE INDEX idx_model_provider ON model(provider_id);

-- API 账号
CREATE TABLE api_account (
    id TEXT PRIMARY KEY,
    provider_id TEXT NOT NULL REFERENCES provider(id) ON DELETE CASCADE,
    label TEXT NOT NULL DEFAULT '',
    api_key_ref TEXT NOT NULL,
    usage_quota INTEGER,
    usage_used INTEGER NOT NULL DEFAULT 0,
    last_used_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_api_account_provider ON api_account(provider_id);

-- 生成任务
CREATE TABLE generation_task (
    id TEXT PRIMARY KEY,
    shot_id TEXT REFERENCES shot(id) ON DELETE SET NULL,
    provider_id TEXT NOT NULL REFERENCES provider(id) ON DELETE RESTRICT,
    model_id TEXT NOT NULL REFERENCES model(id) ON DELETE RESTRICT,
    account_id TEXT NOT NULL REFERENCES api_account(id) ON DELETE RESTRICT,
    task_type TEXT NOT NULL CHECK(task_type IN ('text','image','video','audio')),
    params_json TEXT NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','running','success','failed','cancelled')),
    result_asset_id TEXT REFERENCES asset(id) ON DELETE SET NULL,
    external_task_id TEXT,
    started_at TEXT,
    finished_at TEXT,
    error_message TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_task_shot ON generation_task(shot_id);
CREATE INDEX idx_task_status ON generation_task(status);
CREATE INDEX idx_task_created ON generation_task(created_at);

-- 画布布局
CREATE TABLE canvas_layout (
    id TEXT PRIMARY KEY,
    episode_id TEXT NOT NULL REFERENCES episode(id) ON DELETE CASCADE,
    nodes_json TEXT NOT NULL DEFAULT '[]',
    edges_json TEXT NOT NULL DEFAULT '[]',
    viewport_json TEXT NOT NULL DEFAULT '{}',
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE UNIQUE INDEX idx_canvas_episode ON canvas_layout(episode_id);

-- 集版本检查点
CREATE TABLE episode_checkpoint (
    id TEXT PRIMARY KEY,
    episode_id TEXT NOT NULL REFERENCES episode(id) ON DELETE CASCADE,
    version_number INTEGER NOT NULL,
    label TEXT,
    trigger_type TEXT NOT NULL DEFAULT 'manual' CHECK(trigger_type IN ('auto','manual')),
    script_text TEXT NOT NULL DEFAULT '',
    shots_json TEXT NOT NULL DEFAULT '[]',
    canvas_nodes_json TEXT NOT NULL DEFAULT '[]',
    canvas_edges_json TEXT NOT NULL DEFAULT '[]',
    canvas_viewport_json TEXT NOT NULL DEFAULT '{}',
    change_summary TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_checkpoint_episode ON episode_checkpoint(episode_id);

-- ============================================================
-- 关联表
-- ============================================================

CREATE TABLE shot_character (
    shot_id TEXT NOT NULL REFERENCES shot(id) ON DELETE CASCADE,
    character_id TEXT NOT NULL REFERENCES character_profile(id) ON DELETE CASCADE,
    PRIMARY KEY (shot_id, character_id)
);

CREATE TABLE shot_scene (
    shot_id TEXT NOT NULL REFERENCES shot(id) ON DELETE CASCADE,
    scene_id TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
    PRIMARY KEY (shot_id, scene_id)
);

CREATE TABLE shot_prop (
    shot_id TEXT NOT NULL REFERENCES shot(id) ON DELETE CASCADE,
    prop_id TEXT NOT NULL REFERENCES prop(id) ON DELETE CASCADE,
    PRIMARY KEY (shot_id, prop_id)
);
