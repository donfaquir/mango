pub mod alignment;
pub mod audio;
pub mod commands;
pub mod filter_graph;
pub mod probe;
pub mod progress;
pub mod render;
pub mod sidecar;
pub mod thumbnail;

pub use alignment::{calculate_alignment, calculate_alignment_default, AlignmentStrategy};
pub use audio::{
    mix_audio, overlay_audio_on_video, pad_audio_to_duration, probe_audio_duration, trim_audio,
    AudioMixInput,
};
pub use commands::{
    concat_videos, extract_thumbnail, ms_to_ffmpeg_time, split_video, trim_video, TrimMode,
};
pub use filter_graph::{AmixDuration, FilterGraph};
pub use probe::{probe_video, VideoMetadata};
pub use progress::{FfmpegProgress, FfmpegProgressParser};
pub use render::{render_timeline, RenderConfig, ResolvedTimeline};
pub use sidecar::{check_ffmpeg, FfmpegConfig, FfmpegStatus};
pub use thumbnail::{extract_thumbnail_strip, thumbnail_strip_cache_key, ThumbnailStripResult};
