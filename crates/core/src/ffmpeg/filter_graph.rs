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
}
