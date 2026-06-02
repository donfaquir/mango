ALTER TABLE generation_task ADD COLUMN batch_id TEXT;
CREATE INDEX idx_generation_task_batch ON generation_task(batch_id);
