ALTER TABLE generation_task ADD COLUMN project_id TEXT REFERENCES project(id) ON DELETE CASCADE;
CREATE INDEX idx_generation_task_project_id ON generation_task(project_id);
