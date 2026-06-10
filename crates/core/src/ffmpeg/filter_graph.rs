//! Typed builder for FFmpeg `-filter_complex` strings.
//!
//! Generates filter graph expressions like:
//! `[0:a]volume=0.8[a0];[1:a]adelay=2000|2000[a1];[a0][a1]amix=inputs=2:duration=longest[out]`

use std::fmt::Write;

#[derive(Debug, Clone)]
struct FilterNode {
    inputs: Vec<String>,
    filter: String,
    outputs: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct FilterGraph {
    nodes: Vec<FilterNode>,
}

#[derive(Debug, Clone, Copy)]
pub enum AmixDuration {
    Longest,
    Shortest,
    First,
}

impl AmixDuration {
    fn as_str(self) -> &'static str {
        match self {
            Self::Longest => "longest",
            Self::Shortest => "shortest",
            Self::First => "first",
        }
    }
}

fn escape_drawtext(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace(':', "\\:")
        .replace('\'', "'\\''")
}

impl FilterGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn volume(&mut self, input: &str, vol: f64, output: &str) -> &mut Self {
        self.nodes.push(FilterNode {
            inputs: vec![input.to_string()],
            filter: format!("volume={vol}"),
            outputs: vec![output.to_string()],
        });
        self
    }

    pub fn adelay(&mut self, input: &str, delay_ms: i64, output: &str) -> &mut Self {
        self.nodes.push(FilterNode {
            inputs: vec![input.to_string()],
            filter: format!("adelay={delay_ms}|{delay_ms}"),
            outputs: vec![output.to_string()],
        });
        self
    }

    pub fn apad(&mut self, input: &str, duration_ms: i64, output: &str) -> &mut Self {
        let secs = duration_ms as f64 / 1000.0;
        self.nodes.push(FilterNode {
            inputs: vec![input.to_string()],
            filter: format!("apad=whole_dur={secs}"),
            outputs: vec![output.to_string()],
        });
        self
    }

    pub fn atrim(&mut self, input: &str, end_ms: i64, output: &str) -> &mut Self {
        let secs = end_ms as f64 / 1000.0;
        self.nodes.push(FilterNode {
            inputs: vec![input.to_string()],
            filter: format!("atrim=end={secs}"),
            outputs: vec![output.to_string()],
        });
        self
    }

    pub fn amix(&mut self, inputs: &[&str], duration: AmixDuration, output: &str) -> &mut Self {
        self.nodes.push(FilterNode {
            inputs: inputs.iter().map(|s| s.to_string()).collect(),
            filter: format!("amix=inputs={}:duration={}", inputs.len(), duration.as_str()),
            outputs: vec![output.to_string()],
        });
        self
    }

    pub fn aconcat(&mut self, inputs: &[&str], output: &str) -> &mut Self {
        self.nodes.push(FilterNode {
            inputs: inputs.iter().map(|s| s.to_string()).collect(),
            filter: format!("concat=n={}:v=0:a=1", inputs.len()),
            outputs: vec![output.to_string()],
        });
        self
    }

    // === Video filters ===

    pub fn scale(&mut self, input: &str, width: i32, height: i32, output: &str) -> &mut Self {
        self.nodes.push(FilterNode {
            inputs: vec![input.to_string()],
            filter: format!("scale={width}:{height}"),
            outputs: vec![output.to_string()],
        });
        self
    }

    #[allow(clippy::too_many_arguments)]
    pub fn pad(
        &mut self,
        input: &str,
        width: i32,
        height: i32,
        x: i32,
        y: i32,
        color: &str,
        output: &str,
    ) -> &mut Self {
        self.nodes.push(FilterNode {
            inputs: vec![input.to_string()],
            filter: format!("pad={width}:{height}:{x}:{y}:color={color}"),
            outputs: vec![output.to_string()],
        });
        self
    }

    pub fn fps(&mut self, input: &str, fps: f64, output: &str) -> &mut Self {
        self.nodes.push(FilterNode {
            inputs: vec![input.to_string()],
            filter: format!("fps={fps}"),
            outputs: vec![output.to_string()],
        });
        self
    }

    pub fn format(&mut self, input: &str, pix_fmt: &str, output: &str) -> &mut Self {
        self.nodes.push(FilterNode {
            inputs: vec![input.to_string()],
            filter: format!("format={pix_fmt}"),
            outputs: vec![output.to_string()],
        });
        self
    }

    pub fn normalize(
        &mut self,
        input: &str,
        width: i32,
        height: i32,
        fps: f64,
        pix_fmt: &str,
        output: &str,
    ) -> &mut Self {
        self.nodes.push(FilterNode {
            inputs: vec![input.to_string()],
            filter: format!("scale={width}:{height},fps={fps},format={pix_fmt}"),
            outputs: vec![output.to_string()],
        });
        self
    }

    #[allow(clippy::too_many_arguments)]
    pub fn zoompan(
        &mut self,
        input: &str,
        zoom_expr: &str,
        x_expr: &str,
        y_expr: &str,
        duration_frames: u32,
        width: u32,
        height: u32,
        fps: u32,
        output: &str,
    ) -> &mut Self {
        self.nodes.push(FilterNode {
            inputs: vec![input.to_string()],
            filter: format!(
                "zoompan=z='{zoom_expr}':x='{x_expr}':y='{y_expr}':d={duration_frames}:s={width}x{height}:fps={fps}"
            ),
            outputs: vec![output.to_string()],
        });
        self
    }

    pub fn xfade(
        &mut self,
        input_a: &str,
        input_b: &str,
        transition: &str,
        duration_secs: f64,
        offset_secs: f64,
        output: &str,
    ) -> &mut Self {
        self.nodes.push(FilterNode {
            inputs: vec![input_a.to_string(), input_b.to_string()],
            filter: format!(
                "xfade=transition={transition}:duration={duration_secs}:offset={offset_secs}"
            ),
            outputs: vec![output.to_string()],
        });
        self
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drawtext(
        &mut self,
        input: &str,
        text: &str,
        fontfile: Option<&str>,
        fontsize: u32,
        fontcolor: &str,
        x: &str,
        y: &str,
        enable_expr: Option<&str>,
        output: &str,
    ) -> &mut Self {
        let escaped = escape_drawtext(text);
        let mut f = format!("drawtext=text='{escaped}':fontsize={fontsize}:fontcolor={fontcolor}:x={x}:y={y}");
        if let Some(ff) = fontfile {
            let _ = write!(f, ":fontfile='{ff}'");
        }
        if let Some(expr) = enable_expr {
            let _ = write!(f, ":enable='{expr}'");
        }
        self.nodes.push(FilterNode {
            inputs: vec![input.to_string()],
            filter: f,
            outputs: vec![output.to_string()],
        });
        self
    }

    pub fn overlay(
        &mut self,
        main_input: &str,
        overlay_input: &str,
        x: &str,
        y: &str,
        enable_expr: Option<&str>,
        output: &str,
    ) -> &mut Self {
        let mut f = format!("overlay=x={x}:y={y}");
        if let Some(expr) = enable_expr {
            let _ = write!(f, ":enable='{expr}'");
        }
        self.nodes.push(FilterNode {
            inputs: vec![main_input.to_string(), overlay_input.to_string()],
            filter: f,
            outputs: vec![output.to_string()],
        });
        self
    }

    #[allow(clippy::too_many_arguments)]
    pub fn colorbalance(
        &mut self,
        input: &str,
        rs: f64,
        gs: f64,
        bs: f64,
        rm: f64,
        gm: f64,
        bm: f64,
        rh: f64,
        gh: f64,
        bh: f64,
        output: &str,
    ) -> &mut Self {
        self.nodes.push(FilterNode {
            inputs: vec![input.to_string()],
            filter: format!(
                "colorbalance=rs={rs}:gs={gs}:bs={bs}:rm={rm}:gm={gm}:bm={bm}:rh={rh}:gh={gh}:bh={bh}"
            ),
            outputs: vec![output.to_string()],
        });
        self
    }

    pub fn eq(
        &mut self,
        input: &str,
        brightness: f64,
        contrast: f64,
        saturation: f64,
        output: &str,
    ) -> &mut Self {
        self.nodes.push(FilterNode {
            inputs: vec![input.to_string()],
            filter: format!(
                "eq=brightness={brightness}:contrast={contrast}:saturation={saturation}"
            ),
            outputs: vec![output.to_string()],
        });
        self
    }

    pub fn video_concat(
        &mut self,
        inputs: &[&str],
        n_segments: usize,
        has_audio: bool,
        output_v: &str,
        output_a: Option<&str>,
    ) -> &mut Self {
        let a = if has_audio { 1 } else { 0 };
        let mut outputs = vec![output_v.to_string()];
        if let Some(oa) = output_a {
            outputs.push(oa.to_string());
        }
        self.nodes.push(FilterNode {
            inputs: inputs.iter().map(|s| s.to_string()).collect(),
            filter: format!("concat=n={n_segments}:v=1:a={a}"),
            outputs,
        });
        self
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn build(&self) -> String {
        let mut out = String::new();
        for (i, node) in self.nodes.iter().enumerate() {
            if i > 0 {
                out.push(';');
            }
            for inp in &node.inputs {
                let _ = write!(out, "[{inp}]");
            }
            out.push_str(&node.filter);
            for o in &node.outputs {
                let _ = write!(out, "[{o}]");
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph() {
        let g = FilterGraph::new();
        assert!(g.is_empty());
        assert_eq!(g.build(), "");
    }

    #[test]
    fn single_volume() {
        let mut g = FilterGraph::new();
        g.volume("0:a", 0.8, "a0");
        assert_eq!(g.build(), "[0:a]volume=0.8[a0]");
    }

    #[test]
    fn volume_adelay_chain() {
        let mut g = FilterGraph::new();
        g.volume("0:a", 0.5, "v0");
        g.adelay("v0", 2000, "d0");
        assert_eq!(g.build(), "[0:a]volume=0.5[v0];[v0]adelay=2000|2000[d0]");
    }

    #[test]
    fn amix_two_inputs() {
        let mut g = FilterGraph::new();
        g.amix(&["a0", "a1"], AmixDuration::Longest, "out");
        assert_eq!(g.build(), "[a0][a1]amix=inputs=2:duration=longest[out]");
    }

    #[test]
    fn complex_three_inputs() {
        let mut g = FilterGraph::new();
        g.volume("0:a", 0.8, "v0");
        g.volume("1:a", 0.6, "v1");
        g.adelay("2:a", 1500, "d2");
        g.amix(&["v0", "v1", "d2"], AmixDuration::Longest, "out");
        let result = g.build();
        assert_eq!(
            result,
            "[0:a]volume=0.8[v0];[1:a]volume=0.6[v1];[2:a]adelay=1500|1500[d2];[v0][v1][d2]amix=inputs=3:duration=longest[out]"
        );
    }

    #[test]
    fn aconcat_two_segments() {
        let mut g = FilterGraph::new();
        g.aconcat(&["0:a", "1:a"], "out");
        assert_eq!(g.build(), "[0:a][1:a]concat=n=2:v=0:a=1[out]");
    }

    #[test]
    fn apad_and_atrim() {
        let mut g = FilterGraph::new();
        g.apad("0:a", 5000, "padded");
        g.atrim("padded", 3000, "trimmed");
        assert_eq!(
            g.build(),
            "[0:a]apad=whole_dur=5[padded];[padded]atrim=end=3[trimmed]"
        );
    }

    // === Video filter tests ===

    #[test]
    fn scale_filter() {
        let mut g = FilterGraph::new();
        g.scale("0:v", 1920, 1080, "scaled");
        assert_eq!(g.build(), "[0:v]scale=1920:1080[scaled]");
    }

    #[test]
    fn pad_filter() {
        let mut g = FilterGraph::new();
        g.pad("0:v", 1920, 1080, 0, 60, "black", "padded");
        assert_eq!(
            g.build(),
            "[0:v]pad=1920:1080:0:60:color=black[padded]"
        );
    }

    #[test]
    fn normalize_single_chain() {
        let mut g = FilterGraph::new();
        g.normalize("0:v", 1920, 1080, 30.0, "yuv420p", "norm0");
        assert_eq!(
            g.build(),
            "[0:v]scale=1920:1080,fps=30,format=yuv420p[norm0]"
        );
    }

    #[test]
    fn xfade_dissolve() {
        let mut g = FilterGraph::new();
        g.xfade("v0", "v1", "dissolve", 0.5, 4.5, "xf0");
        assert_eq!(
            g.build(),
            "[v0][v1]xfade=transition=dissolve:duration=0.5:offset=4.5[xf0]"
        );
    }

    #[test]
    fn drawtext_basic() {
        let mut g = FilterGraph::new();
        g.drawtext("0:v", "Hello World", None, 48, "white", "(w-text_w)/2", "h-60", None, "txt");
        assert_eq!(
            g.build(),
            "[0:v]drawtext=text='Hello World':fontsize=48:fontcolor=white:x=(w-text_w)/2:y=h-60[txt]"
        );
    }

    #[test]
    fn drawtext_escape() {
        let mut g = FilterGraph::new();
        g.drawtext("0:v", "time: 3'30\"", None, 24, "yellow", "10", "10", None, "txt");
        let result = g.build();
        assert!(result.contains("time\\: 3'\\''30\""), "got: {result}");
    }

    #[test]
    fn drawtext_with_enable() {
        let mut g = FilterGraph::new();
        g.drawtext("0:v", "Hi", None, 32, "white", "10", "10", Some("between(t,1,5)"), "txt");
        let result = g.build();
        assert!(result.contains("enable='between(t,1,5)'"), "got: {result}");
    }

    #[test]
    fn overlay_basic() {
        let mut g = FilterGraph::new();
        g.overlay("main", "logo", "10", "10", None, "out");
        assert_eq!(g.build(), "[main][logo]overlay=x=10:y=10[out]");
    }

    #[test]
    fn overlay_with_enable() {
        let mut g = FilterGraph::new();
        g.overlay("main", "sticker", "100", "200", Some("between(t,2,8)"), "out");
        let result = g.build();
        assert_eq!(
            result,
            "[main][sticker]overlay=x=100:y=200:enable='between(t,2,8)'[out]"
        );
    }

    #[test]
    fn zoompan_expr() {
        let mut g = FilterGraph::new();
        g.zoompan("0:v", "1+0.001*in", "iw/2", "ih/2", 120, 1920, 1080, 30, "zp");
        let result = g.build();
        assert!(result.contains("zoompan=z='1+0.001*in'"), "got: {result}");
        assert!(result.contains("s=1920x1080"), "got: {result}");
        assert!(result.contains("d=120"), "got: {result}");
    }

    #[test]
    fn colorbalance_params() {
        let mut g = FilterGraph::new();
        g.colorbalance("0:v", 0.1, 0.0, -0.1, 0.0, 0.0, 0.0, -0.1, 0.0, 0.1, "cb");
        let result = g.build();
        assert!(result.contains("rs=0.1"), "got: {result}");
        assert!(result.contains("bs=-0.1"), "got: {result}");
        assert!(result.contains("bh=0.1"), "got: {result}");
    }

    #[test]
    fn eq_params() {
        let mut g = FilterGraph::new();
        g.eq("0:v", 0.05, 1.2, 1.5, "eq0");
        assert_eq!(
            g.build(),
            "[0:v]eq=brightness=0.05:contrast=1.2:saturation=1.5[eq0]"
        );
    }

    #[test]
    fn video_concat_with_audio() {
        let mut g = FilterGraph::new();
        g.video_concat(&["v0", "a0", "v1", "a1"], 2, true, "vout", Some("aout"));
        assert_eq!(
            g.build(),
            "[v0][a0][v1][a1]concat=n=2:v=1:a=1[vout][aout]"
        );
    }

    #[test]
    fn video_concat_video_only() {
        let mut g = FilterGraph::new();
        g.video_concat(&["v0", "v1", "v2"], 3, false, "vout", None);
        assert_eq!(
            g.build(),
            "[v0][v1][v2]concat=n=3:v=1:a=0[vout]"
        );
    }

    #[test]
    fn complex_mixed_graph() {
        let mut g = FilterGraph::new();
        g.normalize("0:v", 1920, 1080, 30.0, "yuv420p", "n0");
        g.normalize("1:v", 1920, 1080, 30.0, "yuv420p", "n1");
        g.normalize("2:v", 1920, 1080, 30.0, "yuv420p", "n2");
        g.xfade("n0", "n1", "dissolve", 0.5, 4.5, "xf0");
        g.xfade("xf0", "n2", "wipeleft", 0.5, 8.5, "xf1");
        g.drawtext("xf1", "Title", None, 64, "white", "(w-text_w)/2", "50", Some("between(t,0,3)"), "txt");
        g.volume("0:a", 0.8, "a0");
        g.volume("1:a", 0.8, "a1");
        g.amix(&["a0", "a1"], AmixDuration::Longest, "aout");

        let result = g.build();
        assert!(result.contains("[n0][n1]xfade"), "missing xfade: {result}");
        assert!(result.contains("[xf0][n2]xfade"), "missing second xfade: {result}");
        assert!(result.contains("drawtext=text='Title'"), "missing drawtext: {result}");
        assert!(result.contains("[a0][a1]amix"), "missing amix: {result}");
        assert_eq!(result.matches(';').count(), 8);
    }
}
