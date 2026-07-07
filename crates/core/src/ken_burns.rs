use crate::error::{CoreError, Result};
use crate::ffmpeg::render::KenBurnsParams;

pub struct KenBurnsPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub start_zoom: f64,
    pub end_zoom: f64,
    pub start_x: f64,
    pub end_x: f64,
    pub start_y: f64,
    pub end_y: f64,
    pub easing: &'static str,
}

static PRESETS: &[KenBurnsPreset] = &[
    KenBurnsPreset {
        id: "slow_zoom_in",
        label: "Slow Zoom In",
        description: "缓慢推进",
        start_zoom: 1.0, end_zoom: 1.15,
        start_x: 0.5, end_x: 0.5,
        start_y: 0.5, end_y: 0.5,
        easing: "linear",
    },
    KenBurnsPreset {
        id: "slow_zoom_out",
        label: "Slow Zoom Out",
        description: "缓慢拉远",
        start_zoom: 1.15, end_zoom: 1.0,
        start_x: 0.5, end_x: 0.5,
        start_y: 0.5, end_y: 0.5,
        easing: "linear",
    },
    KenBurnsPreset {
        id: "pan_left",
        label: "Pan Left",
        description: "左移",
        start_zoom: 1.05, end_zoom: 1.05,
        start_x: 0.55, end_x: 0.45,
        start_y: 0.5, end_y: 0.5,
        easing: "linear",
    },
    KenBurnsPreset {
        id: "pan_right",
        label: "Pan Right",
        description: "右移",
        start_zoom: 1.05, end_zoom: 1.05,
        start_x: 0.45, end_x: 0.55,
        start_y: 0.5, end_y: 0.5,
        easing: "linear",
    },
    KenBurnsPreset {
        id: "pan_up",
        label: "Pan Up",
        description: "上移",
        start_zoom: 1.05, end_zoom: 1.05,
        start_x: 0.5, end_x: 0.5,
        start_y: 0.55, end_y: 0.45,
        easing: "linear",
    },
    KenBurnsPreset {
        id: "pan_down",
        label: "Pan Down",
        description: "下移",
        start_zoom: 1.05, end_zoom: 1.05,
        start_x: 0.5, end_x: 0.5,
        start_y: 0.45, end_y: 0.55,
        easing: "linear",
    },
    KenBurnsPreset {
        id: "zoom_pan_right",
        label: "Zoom + Pan Right",
        description: "推进+右移",
        start_zoom: 1.0, end_zoom: 1.15,
        start_x: 0.45, end_x: 0.55,
        start_y: 0.5, end_y: 0.5,
        easing: "linear",
    },
    KenBurnsPreset {
        id: "dramatic_zoom",
        label: "Dramatic Zoom",
        description: "戏剧推进",
        start_zoom: 1.0, end_zoom: 1.4,
        start_x: 0.5, end_x: 0.5,
        start_y: 0.5, end_y: 0.5,
        easing: "ease_in",
    },
];

pub fn list_presets() -> &'static [KenBurnsPreset] {
    PRESETS
}

pub fn get_preset(id: &str) -> Option<&'static KenBurnsPreset> {
    PRESETS.iter().find(|p| p.id == id)
}

pub fn preset_to_zoompan(preset_id: &str, duration_frames: u32) -> Result<KenBurnsParams> {
    let p = get_preset(preset_id)
        .ok_or_else(|| CoreError::Validation(format!("unknown ken burns preset: {preset_id}")))?;

    let n = duration_frames.saturating_sub(1).max(1);
    let zoom_expr = interp_expr(p.start_zoom, p.end_zoom, n, p.easing);
    let x_expr = pan_expr(p.start_x, p.end_x, n, p.easing, "iw");
    let y_expr = pan_expr(p.start_y, p.end_y, n, p.easing, "ih");

    Ok(KenBurnsParams { zoom_expr, x_expr, y_expr })
}

fn f(v: f64) -> String {
    let s = format!("{:.6}", v);
    let s = s.trim_end_matches('0');
    s.trim_end_matches('.').to_string()
}

fn interp_expr(start: f64, end: f64, n: u32, easing: &str) -> String {
    if (start - end).abs() < 1e-9 {
        return f(start);
    }
    let delta = f(end - start);
    let start = f(start);
    match easing {
        "ease_in" => format!("{start}+{delta}*pow(on/{n},2)"),
        _ => format!("{start}+{delta}*on/{n}"),
    }
}

fn pan_expr(start: f64, end: f64, n: u32, easing: &str, dim: &str) -> String {
    let center = if (start - end).abs() < 1e-9 {
        format!("{}*{dim}", f(start))
    } else {
        let delta = f(end - start);
        let start = f(start);
        match easing {
            "ease_in" => format!("({start}+{delta}*pow(on/{n},2))*{dim}"),
            _ => format!("({start}+{delta}*on/{n})*{dim}"),
        }
    };
    format!("{center}-{dim}/(2*zoom)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_returns_eight_presets() {
        assert_eq!(list_presets().len(), 8);
    }

    #[test]
    fn slow_zoom_in_expr() {
        let kb = preset_to_zoompan("slow_zoom_in", 151).unwrap();
        assert!(kb.zoom_expr.contains("1+0.15"), "got: {}", kb.zoom_expr);
        assert!(kb.zoom_expr.contains("on/150"), "got: {}", kb.zoom_expr);
        assert!(kb.x_expr.contains("0.5*iw"), "got: {}", kb.x_expr);
    }

    #[test]
    fn dramatic_zoom_uses_pow() {
        let kb = preset_to_zoompan("dramatic_zoom", 121).unwrap();
        assert!(kb.zoom_expr.contains("pow(on/120,2)"), "got: {}", kb.zoom_expr);
    }

    #[test]
    fn pan_left_xy() {
        let kb = preset_to_zoompan("pan_left", 91).unwrap();
        assert_eq!(kb.zoom_expr, "1.05");
        assert!(kb.x_expr.starts_with("(0.55"), "got: {}", kb.x_expr);
        assert!(kb.x_expr.contains("-0.1*on/90"), "got: {}", kb.x_expr);
        assert!(kb.y_expr.starts_with("0.5*ih"), "got: {}", kb.y_expr);
    }

    #[test]
    fn unknown_preset_errors() {
        assert!(preset_to_zoompan("nonexistent", 100).is_err());
    }

    #[test]
    fn single_frame_no_division_by_zero() {
        let kb = preset_to_zoompan("slow_zoom_in", 1).unwrap();
        assert!(kb.zoom_expr.contains("on/1"), "got: {}", kb.zoom_expr);
    }
}
