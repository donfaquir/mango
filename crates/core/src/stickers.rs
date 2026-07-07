use std::path::{Path, PathBuf};

use crate::error::CoreError;

pub struct StickerPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub category: &'static str,
    pub filename: &'static str,
    pub default_width: u32,
    pub default_height: u32,
}

static PRESETS: &[StickerPreset] = &[
    StickerPreset { id: "sweat_drop", label: "汗滴", category: "emotion", filename: "sweat_drop.png", default_width: 64, default_height: 64 },
    StickerPreset { id: "anger_vein", label: "怒筋", category: "emotion", filename: "anger_vein.png", default_width: 64, default_height: 64 },
    StickerPreset { id: "question_mark", label: "问号", category: "emotion", filename: "question_mark.png", default_width: 64, default_height: 64 },
    StickerPreset { id: "heart", label: "爱心", category: "emotion", filename: "heart.png", default_width: 64, default_height: 64 },
    StickerPreset { id: "sparkle_eyes", label: "星星眼", category: "emotion", filename: "sparkle_eyes.png", default_width: 64, default_height: 64 },
    StickerPreset { id: "ellipsis", label: "省略号", category: "emotion", filename: "ellipsis.png", default_width: 64, default_height: 64 },
    StickerPreset { id: "shock_lines", label: "冲击线", category: "effect", filename: "shock_lines.png", default_width: 128, default_height: 128 },
    StickerPreset { id: "speed_lines", label: "速度线", category: "effect", filename: "speed_lines.png", default_width: 128, default_height: 128 },
    StickerPreset { id: "light_burst", label: "闪光", category: "effect", filename: "light_burst.png", default_width: 128, default_height: 128 },
    StickerPreset { id: "comic_frame", label: "漫画框", category: "frame", filename: "comic_frame.png", default_width: 128, default_height: 128 },
];

pub fn list_presets() -> &'static [StickerPreset] {
    PRESETS
}

pub fn get_preset(id: &str) -> Option<&'static StickerPreset> {
    PRESETS.iter().find(|p| p.id == id)
}

pub fn resolve_sticker_path(sticker_id: &str, assets_dir: &Path) -> Result<PathBuf, CoreError> {
    let preset = get_preset(sticker_id)
        .ok_or_else(|| CoreError::Validation(format!("unknown sticker id: {sticker_id}")))?;
    let path = assets_dir.join("stickers").join(preset.filename);
    if !path.exists() {
        return Err(CoreError::Validation(format!(
            "sticker file not found: {}",
            path.display()
        )));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_returns_10() {
        assert_eq!(list_presets().len(), 10);
    }

    #[test]
    fn get_by_id() {
        let p = get_preset("heart").unwrap();
        assert_eq!(p.label, "爱心");
        assert_eq!(p.category, "emotion");
    }

    #[test]
    fn get_unknown_none() {
        assert!(get_preset("nonexistent").is_none());
    }
}
