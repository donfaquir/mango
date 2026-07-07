use crate::ffmpeg::render::ColorEffectParams;

pub struct ColorPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub effect: ColorEffectParams,
}

static PRESETS: &[ColorPreset] = &[
    ColorPreset {
        id: "warm", label: "暖调",
        effect: ColorEffectParams::Colorbalance {
            rs: 0.1, gs: 0.0, bs: -0.1, rm: 0.0, gm: 0.0, bm: 0.0, rh: 0.0, gh: 0.0, bh: 0.0,
        },
    },
    ColorPreset {
        id: "cool", label: "冷调",
        effect: ColorEffectParams::Colorbalance {
            rs: -0.1, gs: 0.0, bs: 0.1, rm: 0.0, gm: 0.0, bm: 0.0, rh: 0.0, gh: 0.0, bh: 0.0,
        },
    },
    ColorPreset {
        id: "vintage", label: "怀旧",
        effect: ColorEffectParams::Eq { brightness: 0.0, contrast: 1.0, saturation: 0.7 },
    },
    ColorPreset {
        id: "bw", label: "黑白",
        effect: ColorEffectParams::Eq { brightness: 0.0, contrast: 1.0, saturation: 0.0 },
    },
    ColorPreset {
        id: "high_contrast", label: "高对比",
        effect: ColorEffectParams::Eq { brightness: 0.0, contrast: 1.3, saturation: 1.1 },
    },
    ColorPreset {
        id: "low_saturation", label: "低饱和",
        effect: ColorEffectParams::Eq { brightness: 0.0, contrast: 1.0, saturation: 0.5 },
    },
    ColorPreset {
        id: "night", label: "夜景",
        effect: ColorEffectParams::Eq { brightness: -0.1, contrast: 1.0, saturation: 1.0 },
    },
    ColorPreset {
        id: "sunset", label: "黄昏",
        effect: ColorEffectParams::Colorbalance {
            rs: 0.15, gs: 0.05, bs: -0.1, rm: 0.0, gm: 0.0, bm: 0.0, rh: 0.0, gh: 0.0, bh: 0.0,
        },
    },
    ColorPreset {
        id: "dreamy", label: "梦幻",
        effect: ColorEffectParams::Eq { brightness: 0.05, contrast: 0.9, saturation: 0.8 },
    },
    ColorPreset {
        id: "horror", label: "恐怖",
        effect: ColorEffectParams::Eq { brightness: 0.0, contrast: 1.2, saturation: 0.3 },
    },
];

pub fn list_presets() -> &'static [ColorPreset] {
    PRESETS
}

pub fn get_preset(id: &str) -> Option<&'static ColorPreset> {
    PRESETS.iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_returns_10() {
        assert_eq!(list_presets().len(), 10);
    }

    #[test]
    fn warm_is_colorbalance() {
        let p = get_preset("warm").unwrap();
        assert!(matches!(p.effect, ColorEffectParams::Colorbalance { rs, .. } if (rs - 0.1).abs() < f64::EPSILON));
    }

    #[test]
    fn bw_saturation_zero() {
        let p = get_preset("bw").unwrap();
        assert!(matches!(p.effect, ColorEffectParams::Eq { saturation, .. } if saturation.abs() < f64::EPSILON));
    }
}
