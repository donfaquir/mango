pub struct TransitionPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub xfade_name: &'static str,
}

static PRESETS: &[TransitionPreset] = &[
    TransitionPreset { id: "dissolve", label: "叠化", xfade_name: "dissolve" },
    TransitionPreset { id: "fade_black", label: "淡入黑场", xfade_name: "fade" },
    TransitionPreset { id: "fade_white", label: "淡入白场", xfade_name: "fadewhite" },
    TransitionPreset { id: "wipe_left", label: "左擦除", xfade_name: "wipeleft" },
    TransitionPreset { id: "wipe_right", label: "右擦除", xfade_name: "wiperight" },
    TransitionPreset { id: "circle_open", label: "圆形展开", xfade_name: "circleopen" },
    TransitionPreset { id: "circle_close", label: "圆形收缩", xfade_name: "circleclose" },
    TransitionPreset { id: "pixelize", label: "像素化", xfade_name: "pixelize" },
];

pub fn list_presets() -> &'static [TransitionPreset] {
    PRESETS
}

pub fn get_preset(id: &str) -> Option<&'static TransitionPreset> {
    PRESETS.iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_returns_eight() {
        assert_eq!(list_presets().len(), 8);
    }

    #[test]
    fn get_dissolve() {
        let p = get_preset("dissolve").unwrap();
        assert_eq!(p.xfade_name, "dissolve");
        assert_eq!(p.label, "叠化");
    }

    #[test]
    fn get_unknown_none() {
        assert!(get_preset("nonexistent").is_none());
    }
}
