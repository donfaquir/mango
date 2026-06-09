use rusqlite::Connection;

use crate::db::queries::character as character_queries;
use crate::db::queries::episode as episode_queries;
use crate::db::queries::shot as shot_queries;
use crate::error::Result;
use crate::models::generation_task::{CreateGenerationTaskInput, TaskKind};
use crate::models::shot::ListShotsOptions;

/// Build a TTS task input for a single shot.
///
/// Returns `None` if the shot has no dialogue or its first linked character
/// has no `voice_id` set.
pub fn build_shot_voice_task(
    conn: &Connection,
    shot_id: &str,
    provider_id: &str,
    model_id: &str,
    account_id: &str,
) -> Result<Option<CreateGenerationTaskInput>> {
    let shot = shot_queries::get_by_id(conn, shot_id)?;

    if shot.dialogue.trim().is_empty() {
        return Ok(None);
    }

    let links = shot_queries::list_links(conn, shot_id)?;
    let character_id = match links.character_ids.first() {
        Some(id) => id,
        None => return Ok(None),
    };

    let character = character_queries::get_by_id(conn, character_id)?;
    let voice_id = match character.voice_id {
        Some(ref v) if !v.is_empty() => v.clone(),
        _ => return Ok(None),
    };

    let episode = episode_queries::get_by_id(conn, &shot.episode_id)?;

    let params = serde_json::json!({
        "text": shot.dialogue,
        "voice_id": voice_id,
        "rate": 1.0,
        "volume": 50,
        "pitch": 0,
        "format": "mp3",
        "sample_rate": 24000,
    });

    Ok(Some(CreateGenerationTaskInput {
        project_id: Some(episode.project_id),
        shot_id: Some(shot.id),
        provider_id: provider_id.to_string(),
        model_id: model_id.to_string(),
        account_id: account_id.to_string(),
        task_type: TaskKind::Audio,
        params_json: Some(params.to_string()),
    }))
}

/// Build TTS task inputs for all shots in an episode that have dialogue
/// and a linked character with a voice_id.
pub fn build_episode_voice_tasks(
    conn: &Connection,
    episode_id: &str,
    provider_id: &str,
    model_id: &str,
    account_id: &str,
) -> Result<Vec<CreateGenerationTaskInput>> {
    let shots = shot_queries::list(conn, ListShotsOptions {
        episode_id: episode_id.to_string(),
    })?;

    let mut tasks = Vec::new();
    for shot in &shots {
        if let Some(input) = build_shot_voice_task(conn, &shot.id, provider_id, model_id, account_id)? {
            tasks.push(input);
        }
    }
    Ok(tasks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use crate::db::queries::episode as episode_queries;
    use crate::db::queries::project as project_queries;
    use crate::models::character::CreateCharacterInput;
    use crate::models::episode::CreateEpisodeInput;
    use crate::models::shot::{CreateShotInput, SubjectKind, UpdateShotInput};
    use crate::models::project::CreateProjectInput;
    use std::path::Path;
    use tempfile::tempdir;

    fn setup() -> (Connection, String, String) {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        let td = tempdir().unwrap();
        let project = project_queries::create(
            &conn,
            td.path(),
            CreateProjectInput {
                name: "P".into(),
                subdir: None,
                description: None,
                style_prompt: None,
                global_seed: None,
            },
        )
        .unwrap();
        let episode = episode_queries::create(
            &conn,
            CreateEpisodeInput {
                project_id: project.id.clone(),
                title: "E1".into(),
                script_text: None,
            },
        )
        .unwrap();
        // Leak tempdir so it doesn't get cleaned up during test
        std::mem::forget(td);
        (conn, project.id, episode.id)
    }

    fn create_shot_with_dialogue(conn: &Connection, episode_id: &str, dialogue: &str) -> String {
        let shot = shot_queries::create(
            conn,
            CreateShotInput {
                episode_id: episode_id.to_string(),
                summary: Some("test".into()),
            },
        )
        .unwrap();
        if !dialogue.is_empty() {
            shot_queries::update(
                conn,
                &shot.id,
                UpdateShotInput {
                    dialogue: Some(dialogue.to_string()),
                    ..Default::default()
                },
            )
            .unwrap();
        }
        shot.id
    }

    fn create_character_with_voice(
        conn: &Connection,
        project_id: &str,
        voice_id: Option<&str>,
    ) -> String {
        let c = character_queries::create(
            conn,
            CreateCharacterInput {
                project_id: project_id.to_string(),
                name: "Char".into(),
                description: None,
                appearance_prompt: None,
                reference_image_path: None,
                voice_id: voice_id.map(|s| s.to_string()),
            },
        )
        .unwrap();
        c.id
    }

    #[test]
    fn no_dialogue_returns_none() {
        let (conn, pid, eid) = setup();
        let sid = create_shot_with_dialogue(&conn, &eid, "");
        let cid = create_character_with_voice(&conn, &pid, Some("longxiaochun"));
        shot_queries::link_subject(&conn, &sid, &cid, SubjectKind::Character).unwrap();

        let result = build_shot_voice_task(&conn, &sid, "bailian", "cosyvoice-v2", "acc1").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn no_character_returns_none() {
        let (conn, _pid, eid) = setup();
        let sid = create_shot_with_dialogue(&conn, &eid, "Hello world");

        let result = build_shot_voice_task(&conn, &sid, "bailian", "cosyvoice-v2", "acc1").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn no_voice_id_returns_none() {
        let (conn, pid, eid) = setup();
        let sid = create_shot_with_dialogue(&conn, &eid, "Hello world");
        let cid = create_character_with_voice(&conn, &pid, None);
        shot_queries::link_subject(&conn, &sid, &cid, SubjectKind::Character).unwrap();

        let result = build_shot_voice_task(&conn, &sid, "bailian", "cosyvoice-v2", "acc1").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn valid_shot_returns_task_input() {
        let (conn, pid, eid) = setup();
        let sid = create_shot_with_dialogue(&conn, &eid, "Hello world");
        let cid = create_character_with_voice(&conn, &pid, Some("longshu"));
        shot_queries::link_subject(&conn, &sid, &cid, SubjectKind::Character).unwrap();

        let result = build_shot_voice_task(&conn, &sid, "bailian", "cosyvoice-v2", "acc1").unwrap();
        assert!(result.is_some());
        let input = result.unwrap();
        assert_eq!(input.task_type, TaskKind::Audio);
        assert_eq!(input.provider_id, "bailian");
        assert_eq!(input.model_id, "cosyvoice-v2");
        assert!(input.params_json.as_ref().unwrap().contains("longshu"));
        assert!(input.params_json.as_ref().unwrap().contains("Hello world"));
    }

    #[test]
    fn episode_tasks_filters_correctly() {
        let (conn, pid, eid) = setup();
        let s1 = create_shot_with_dialogue(&conn, &eid, "Line one");
        let s2 = create_shot_with_dialogue(&conn, &eid, "");
        let s3 = create_shot_with_dialogue(&conn, &eid, "Line three");

        let cid = create_character_with_voice(&conn, &pid, Some("longxiaochun"));
        shot_queries::link_subject(&conn, &s1, &cid, SubjectKind::Character).unwrap();
        // s2 has no dialogue — skipped
        shot_queries::link_subject(&conn, &s2, &cid, SubjectKind::Character).unwrap();
        shot_queries::link_subject(&conn, &s3, &cid, SubjectKind::Character).unwrap();

        let tasks = build_episode_voice_tasks(&conn, &eid, "bailian", "cosyvoice-v2", "acc1").unwrap();
        assert_eq!(tasks.len(), 2);
    }
}
