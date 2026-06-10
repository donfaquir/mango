use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextType {
    Subtitle,
    Bubble,
    Fancy,
    Onomatopoeia,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TimelineItemParams {
    Clip {
        ken_burns_preset: Option<String>,
    },
    Text {
        text_type: TextType,
        content: String,
        #[serde(default)]
        style: serde_json::Value,
        #[serde(default)]
        animation: Option<serde_json::Value>,
    },
    Transition {
        transition_type: String,
        #[serde(default)]
        duration_ms: i64,
    },
    Sticker {
        sticker_id: Option<String>,
        custom_path: Option<String>,
        animation: Option<String>,
    },
    Effect {
        effect_type: String,
        #[serde(default)]
        params: serde_json::Value,
    },
}

pub fn parse_params(json: &str) -> Result<TimelineItemParams> {
    serde_json::from_str(json)
        .map_err(|e| CoreError::Validation(format!("invalid params_json: {e}")))
}

pub fn serialize_params(params: &TimelineItemParams) -> Result<String> {
    serde_json::to_string(params)
        .map_err(|e| CoreError::Validation(format!("failed to serialize params: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_roundtrip() {
        let p = TimelineItemParams::Clip { ken_burns_preset: Some("slow_zoom".into()) };
        let json = serialize_params(&p).unwrap();
        let parsed = parse_params(&json).unwrap();
        assert!(matches!(parsed, TimelineItemParams::Clip { ken_burns_preset: Some(ref s) } if s == "slow_zoom"));
    }

    #[test]
    fn text_roundtrip() {
        let json = r#"{"type":"Text","text_type":"subtitle","content":"Hello","style":{}}"#;
        let parsed = parse_params(json).unwrap();
        assert!(matches!(parsed, TimelineItemParams::Text { ref content, .. } if content == "Hello"));
    }

    #[test]
    fn transition_roundtrip() {
        let p = TimelineItemParams::Transition { transition_type: "dissolve".into(), duration_ms: 500 };
        let json = serialize_params(&p).unwrap();
        let parsed = parse_params(&json).unwrap();
        assert!(matches!(parsed, TimelineItemParams::Transition { duration_ms: 500, .. }));
    }

    #[test]
    fn invalid_json_rejected() {
        assert!(parse_params("not json").is_err());
    }

    #[test]
    fn unknown_type_rejected() {
        assert!(parse_params(r#"{"type":"Unknown"}"#).is_err());
    }
}
