pub struct AspectRatioPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub width: u32,
    pub height: u32,
}

pub struct PlatformPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub width: u32,
    pub height: u32,
    pub codec: &'static str,
    pub preset: &'static str,
    pub crf: u32,
}

static ASPECT_RATIOS: &[AspectRatioPreset] = &[
    AspectRatioPreset { id: "16_9", label: "16:9 横屏", width: 1920, height: 1080 },
    AspectRatioPreset { id: "9_16", label: "9:16 竖屏", width: 1080, height: 1920 },
    AspectRatioPreset { id: "1_1", label: "1:1 方形", width: 1080, height: 1080 },
    AspectRatioPreset { id: "3_4", label: "3:4 竖版", width: 1080, height: 1440 },
];

static PLATFORM_PRESETS: &[PlatformPreset] = &[
    PlatformPreset { id: "douyin", label: "抖音", width: 1080, height: 1920, codec: "libx264", preset: "fast", crf: 18 },
    PlatformPreset { id: "bilibili", label: "B站", width: 1920, height: 1080, codec: "libx264", preset: "fast", crf: 18 },
    PlatformPreset { id: "wechat_h", label: "微信横版", width: 1920, height: 1080, codec: "libx264", preset: "fast", crf: 20 },
    PlatformPreset { id: "wechat_v", label: "微信竖版", width: 1080, height: 1920, codec: "libx264", preset: "fast", crf: 20 },
    PlatformPreset { id: "xiaohongshu", label: "小红书", width: 1080, height: 1440, codec: "libx264", preset: "fast", crf: 18 },
];

pub fn list_aspect_ratios() -> &'static [AspectRatioPreset] {
    ASPECT_RATIOS
}

pub fn list_platform_presets() -> &'static [PlatformPreset] {
    PLATFORM_PRESETS
}

pub fn get_platform_preset(id: &str) -> Option<&'static PlatformPreset> {
    PLATFORM_PRESETS.iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aspect_ratio_count() {
        assert_eq!(list_aspect_ratios().len(), 4);
    }

    #[test]
    fn platform_presets_count() {
        assert_eq!(list_platform_presets().len(), 5);
    }

    #[test]
    fn douyin_is_vertical() {
        let p = get_platform_preset("douyin").unwrap();
        assert_eq!(p.width, 1080);
        assert_eq!(p.height, 1920);
        assert_eq!(p.codec, "libx264");
    }
}
