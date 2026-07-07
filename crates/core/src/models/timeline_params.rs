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

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TextStyle {
    #[serde(default)]
    pub font_family: Option<String>,
    #[serde(default)]
    pub font_size: Option<u32>,
    #[serde(default)]
    pub font_weight: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub outline_color: Option<String>,
    #[serde(default)]
    pub outline_width: Option<u32>,
    #[serde(default)]
    pub shadow: Option<bool>,
    #[serde(default)]
    pub position_x: Option<f64>,
    #[serde(default)]
    pub position_y: Option<f64>,
    #[serde(default)]
    pub alignment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct BubbleStyle {
    #[serde(default)]
    pub shape: Option<String>,
    #[serde(default)]
    pub tail_direction: Option<String>,
    #[serde(default)]
    pub fill_color: Option<String>,
    #[serde(default)]
    pub border_color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TextAnimation {
    #[serde(default)]
    pub enter: Option<String>,
    #[serde(default)]
    pub exit: Option<String>,
    #[serde(default)]
    pub enter_duration_ms: Option<i64>,
    #[serde(default)]
    pub exit_duration_ms: Option<i64>,
}

pub fn default_text_style(text_type: &TextType) -> TextStyle {
    match text_type {
        TextType::Subtitle => TextStyle {
            font_size: Some(36),
            color: Some("#FFFFFF".into()),
            outline_color: Some("#000000".into()),
            outline_width: Some(2),
            position_y: Some(0.9),
            alignment: Some("center".into()),
            ..Default::default()
        },
        TextType::Bubble => TextStyle {
            font_size: Some(28),
            color: Some("#000000".into()),
            position_x: Some(0.5),
            position_y: Some(0.5),
            ..Default::default()
        },
        TextType::Fancy => TextStyle {
            font_size: Some(72),
            font_weight: Some("bold".into()),
            color: Some("#FFD700".into()),
            outline_color: Some("#FF4500".into()),
            outline_width: Some(3),
            position_y: Some(0.3),
            alignment: Some("center".into()),
            ..Default::default()
        },
        TextType::Onomatopoeia => TextStyle {
            font_size: Some(96),
            font_weight: Some("bold".into()),
            color: Some("#FF0000".into()),
            outline_color: Some("#000000".into()),
            outline_width: Some(4),
            position_x: Some(0.5),
            position_y: Some(0.4),
            ..Default::default()
        },
    }
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
        style: Box<TextStyle>,
        #[serde(default)]
        bubble: Option<BubbleStyle>,
        #[serde(default)]
        animation: Option<TextAnimation>,
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

    #[test]
    fn text_style_default_deserialize() {
        let s: TextStyle = serde_json::from_str("{}").unwrap();
        assert_eq!(s, TextStyle::default());
    }

    #[test]
    fn text_style_full_roundtrip() {
        let s = TextStyle {
            font_family: Some("Noto Sans SC".into()),
            font_size: Some(48),
            font_weight: Some("bold".into()),
            color: Some("#FF0000".into()),
            outline_color: Some("#000000".into()),
            outline_width: Some(3),
            shadow: Some(true),
            position_x: Some(0.5),
            position_y: Some(0.9),
            alignment: Some("center".into()),
        };
        let json = serde_json::to_string(&s).unwrap();
        let parsed: TextStyle = serde_json::from_str(&json).unwrap();
        assert_eq!(s, parsed);
    }

    #[test]
    fn bubble_style_roundtrip() {
        let b = BubbleStyle {
            shape: Some("oval".into()),
            tail_direction: Some("bottom_left".into()),
            fill_color: Some("#FFFFFF".into()),
            border_color: Some("#000000".into()),
        };
        let json = serde_json::to_string(&b).unwrap();
        let parsed: BubbleStyle = serde_json::from_str(&json).unwrap();
        assert_eq!(b, parsed);
    }

    #[test]
    fn backward_compat_old_text_params() {
        let json = r#"{"type":"Text","text_type":"subtitle","content":"Hello"}"#;
        let parsed = parse_params(json).unwrap();
        match parsed {
            TimelineItemParams::Text { content, style, bubble, animation, .. } => {
                assert_eq!(content, "Hello");
                assert_eq!(*style, TextStyle::default());
                assert!(bubble.is_none());
                assert!(animation.is_none());
            }
            _ => panic!("expected Text variant"),
        }
    }
}
