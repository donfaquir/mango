use super::{require_mount, with_db};
use crate::error::IpcError;
use crate::state::AppState;
use mango_core::models::generation_task::{CreateGenerationTaskInput, TaskKind};
use mango_core::task_engine::SubmitBatchOutcome;
use mango_core::voice::generation;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn generate_shot_voice(
    state: State<'_, AppState>,
    shot_id: String,
    account_id: String,
) -> Result<String, IpcError> {
    let input = with_db(&state, move |conn| {
        generation::build_shot_voice_task(conn, &shot_id, "bailian", "cosyvoice-v2", &account_id)
    })
    .await?
    .ok_or_else(|| {
        IpcError::from(mango_core::error::CoreError::Validation(
            "该分镜无对白或角色未配置音色".into(),
        ))
    })?;

    let engine = require_mount(&state)?.task_engine.clone();
    engine.submit(input).await.map_err(IpcError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn generate_episode_voices(
    state: State<'_, AppState>,
    episode_id: String,
    account_id: String,
) -> Result<SubmitBatchOutcome, IpcError> {
    let inputs = with_db(&state, move |conn| {
        generation::build_episode_voice_tasks(conn, &episode_id, "bailian", "cosyvoice-v2", &account_id)
    })
    .await?;

    if inputs.is_empty() {
        return Err(IpcError::from(mango_core::error::CoreError::Validation(
            "没有可生成配音的分镜（需有对白且角色已配置音色）".into(),
        )));
    }

    let engine = require_mount(&state)?.task_engine.clone();
    engine.submit_batch(inputs).await.map_err(IpcError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn preview_voice(
    state: State<'_, AppState>,
    voice_id: String,
    text: String,
    account_id: String,
) -> Result<String, IpcError> {
    let params = serde_json::json!({
        "text": text,
        "voice_id": voice_id,
        "rate": 1.0,
        "volume": 50,
        "pitch": 0,
        "format": "mp3",
        "sample_rate": 24000,
    });

    let input = CreateGenerationTaskInput {
        project_id: None,
        shot_id: None,
        provider_id: "bailian".to_string(),
        model_id: "cosyvoice-v2".to_string(),
        account_id,
        task_type: TaskKind::Audio,
        params_json: Some(params.to_string()),
    };

    let engine = require_mount(&state)?.task_engine.clone();
    engine.submit(input).await.map_err(IpcError::from)
}
