use crate::error::{CoreError, Result};
use crate::provider::error::{ProviderErrorDetail, ProviderErrorKind};
use crate::provider::traits::GenerationParams;

pub(super) fn validate_params(params: &GenerationParams) -> Result<()> {
    match params.model_id.as_str() {
        "wan2.7-image-pro" => validate_wan27(params),
        "happyhorse-1.0-r2v" => validate_happyhorse(params),
        "cosyvoice-v2" => validate_cosyvoice(params),
        other => Err(CoreError::Provider(ProviderErrorDetail::new(
            ProviderErrorKind::InvalidRequest,
            format!("不支持的百炼模型: {other}"),
        ))),
    }
}

fn validate_wan27(params: &GenerationParams) -> Result<()> {
    if params.prompt.trim().is_empty() {
        return invalid("wan2.7-image-pro 需要非空 prompt");
    }
    let size = params
        .provider_params
        .get("size")
        .and_then(|v| v.as_str())
        .unwrap_or("2K");
    match size {
        "1K" | "2K" => {}
        _ => return invalid("wan2.7-image-pro 的 size 仅支持 1K/2K"),
    }
    let n = params
        .provider_params
        .get("n")
        .and_then(|v| v.as_u64())
        .unwrap_or(1);
    if !(1..=4).contains(&n) {
        return invalid("wan2.7-image-pro 的 n 仅支持 1~4");
    }
    Ok(())
}

fn validate_happyhorse(params: &GenerationParams) -> Result<()> {
    if params.prompt.trim().is_empty() {
        return invalid("happyhorse-1.0-r2v 需要非空 prompt");
    }
    let media = params
        .provider_params
        .get("media")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            CoreError::Provider(ProviderErrorDetail::new(
                ProviderErrorKind::InvalidRequest,
                "happyhorse-1.0-r2v 需要 media[] 参考图参数",
            ))
        })?;
    if media.is_empty() {
        return invalid("happyhorse-1.0-r2v 至少需要一张参考图");
    }

    let resolution = params
        .provider_params
        .get("resolution")
        .and_then(|v| v.as_str())
        .unwrap_or("720P");
    match resolution {
        "720P" | "1080P" => {}
        _ => return invalid("happyhorse-1.0-r2v 的 resolution 仅支持 720P/1080P"),
    }
    let ratio = params
        .provider_params
        .get("ratio")
        .and_then(|v| v.as_str())
        .unwrap_or("16:9");
    match ratio {
        "16:9" | "9:16" | "1:1" => {}
        _ => return invalid("happyhorse-1.0-r2v 的 ratio 仅支持 16:9/9:16/1:1"),
    }
    let duration = params
        .provider_params
        .get("duration")
        .and_then(|v| v.as_u64())
        .unwrap_or(5);
    match duration {
        5 | 10 => {}
        _ => return invalid("happyhorse-1.0-r2v 的 duration 仅支持 5 或 10 秒"),
    }
    Ok(())
}

fn validate_cosyvoice(params: &GenerationParams) -> Result<()> {
    let text = params
        .provider_params
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or(params.prompt.as_str());
    if text.trim().is_empty() {
        return invalid("cosyvoice-v2 需要非空文本（provider_params.text 或 prompt）");
    }
    if text.len() > 2000 {
        return invalid("cosyvoice-v2 单次文本长度不能超过 2000 字符");
    }

    let voice_id = params
        .provider_params
        .get("voice_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if voice_id.trim().is_empty() {
        return invalid("cosyvoice-v2 需要指定 voice_id");
    }

    if let Some(rate) = params.provider_params.get("rate").and_then(|v| v.as_f64())
        && !(0.5..=2.0).contains(&rate)
    {
        return invalid("cosyvoice-v2 的 rate 需在 0.5~2.0 之间");
    }
    if let Some(volume) = params.provider_params.get("volume").and_then(|v| v.as_i64())
        && !(0..=100).contains(&volume)
    {
        return invalid("cosyvoice-v2 的 volume 需在 0~100 之间");
    }
    if let Some(pitch) = params.provider_params.get("pitch").and_then(|v| v.as_i64())
        && !(-500..=500).contains(&pitch)
    {
        return invalid("cosyvoice-v2 的 pitch 需在 -500~500 之间");
    }
    Ok(())
}

fn invalid(message: &str) -> Result<()> {
    Err(CoreError::Provider(ProviderErrorDetail::new(
        ProviderErrorKind::InvalidRequest,
        message,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::traits::ProviderCredentials;

    fn base_params(model_id: &str, prompt: &str, provider_params: serde_json::Value) -> GenerationParams {
        GenerationParams {
            model_id: model_id.to_string(),
            prompt: prompt.to_string(),
            provider_params,
            credentials: ProviderCredentials {
                api_key: "k".into(),
                extra_json: Some("{}".into()),
            },
        }
    }

    #[test]
    fn validates_wan27_ok() {
        let p = base_params(
            "wan2.7-image-pro",
            "test",
            serde_json::json!({ "size": "2K", "n": 1 }),
        );
        assert!(validate_params(&p).is_ok());
    }

    #[test]
    fn rejects_wan27_invalid_n() {
        let p = base_params(
            "wan2.7-image-pro",
            "test",
            serde_json::json!({ "size": "2K", "n": 9 }),
        );
        assert!(validate_params(&p).is_err());
    }

    #[test]
    fn validates_happyhorse_ok() {
        let p = base_params(
            "happyhorse-1.0-r2v",
            "test",
            serde_json::json!({
                "media": [{ "asset_id": "a1" }],
                "resolution": "720P",
                "ratio": "16:9",
                "duration": 5
            }),
        );
        assert!(validate_params(&p).is_ok());
    }

    #[test]
    fn rejects_happyhorse_without_media() {
        let p = base_params(
            "happyhorse-1.0-r2v",
            "test",
            serde_json::json!({ "media": [] }),
        );
        assert!(validate_params(&p).is_err());
    }

    #[test]
    fn validates_cosyvoice_ok() {
        let p = base_params(
            "cosyvoice-v2",
            "hello",
            serde_json::json!({ "voice_id": "longxiaochun", "rate": 1.0, "volume": 50 }),
        );
        assert!(validate_params(&p).is_ok());
    }

    #[test]
    fn validates_cosyvoice_text_from_provider_params() {
        let p = base_params(
            "cosyvoice-v2",
            "",
            serde_json::json!({ "text": "from params", "voice_id": "longshu" }),
        );
        assert!(validate_params(&p).is_ok());
    }

    #[test]
    fn rejects_cosyvoice_empty_text() {
        let p = base_params(
            "cosyvoice-v2",
            "",
            serde_json::json!({ "voice_id": "longxiaochun" }),
        );
        assert!(validate_params(&p).is_err());
    }

    #[test]
    fn rejects_cosyvoice_missing_voice() {
        let p = base_params(
            "cosyvoice-v2",
            "hello",
            serde_json::json!({}),
        );
        assert!(validate_params(&p).is_err());
    }

    #[test]
    fn rejects_cosyvoice_rate_out_of_range() {
        let p = base_params(
            "cosyvoice-v2",
            "hello",
            serde_json::json!({ "voice_id": "longshu", "rate": 3.0 }),
        );
        assert!(validate_params(&p).is_err());
    }

    #[test]
    fn rejects_cosyvoice_volume_out_of_range() {
        let p = base_params(
            "cosyvoice-v2",
            "hello",
            serde_json::json!({ "voice_id": "longshu", "volume": 150 }),
        );
        assert!(validate_params(&p).is_err());
    }

    #[test]
    fn rejects_cosyvoice_pitch_out_of_range() {
        let p = base_params(
            "cosyvoice-v2",
            "hello",
            serde_json::json!({ "voice_id": "longshu", "pitch": 600 }),
        );
        assert!(validate_params(&p).is_err());
    }
}
