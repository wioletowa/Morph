//! Audio and video through ffmpeg: probing, command lines, progress parsing.

use crate::formats::{self, Kind};
use crate::gpu::{self, Accel};
use crate::i18n::tr;
use std::ffi::OsString;
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Probe {
    pub duration: Option<f64>,
    pub width: u32,
    pub height: u32,
    /// A real video stream, not just embedded cover art.
    pub has_video: bool,
    pub has_audio: bool,
    pub has_cover: bool,
    /// Codec and pixel format of the main video stream, e.g. "h264" and "yuv420p".
    pub vcodec: String,
    pub pix_fmt: String,
}

#[derive(Debug, Clone)]
pub struct MediaOptions {
    /// "h264", "h265" or "av1".
    pub codec: String,
    /// x264-style CRF; other encoders get an equivalent value.
    pub crf: u32,
    /// "fast", "medium" or "slow".
    pub speed: String,
    /// 0 keeps the original size; otherwise caps the shorter side.
    pub max_height: u32,
    pub audio_kbps: u32,
}

pub fn probe_argv(ffprobe: &Path, src: &Path) -> Vec<OsString> {
    let mut v: Vec<OsString> = ["-v", "error", "-show_entries", "format=duration:stream=codec_type,codec_name,pix_fmt,width,height:stream_disposition=attached_pic", "-of", "flat"]
        .iter()
        .map(OsString::from)
        .collect();
    v.insert(0, ffprobe.into());
    v.push(src.into());
    v
}

/// Parses `ffprobe -of flat` output.
pub fn parse_probe(text: &str) -> Probe {
    let mut p = Probe::default();
    #[derive(Default)]
    struct Stream {
        ty: String,
        codec: String,
        pix_fmt: String,
        width: u32,
        height: u32,
        cover: bool,
    }
    let mut streams: std::collections::BTreeMap<u32, Stream> = Default::default();
    for line in text.lines() {
        let Some((key, val)) = line.split_once('=') else { continue };
        let val = val.trim_matches('"');
        if key == "format.duration" {
            p.duration = val.parse().ok().filter(|d: &f64| d.is_finite() && *d > 0.0);
            continue;
        }
        let Some(rest) = key.strip_prefix("streams.stream.") else { continue };
        let Some((idx, field)) = rest.split_once('.') else { continue };
        let Ok(idx) = idx.parse() else { continue };
        let s = streams.entry(idx).or_default();
        match field {
            "codec_type" => s.ty = val.to_string(),
            "codec_name" => s.codec = val.to_string(),
            "pix_fmt" => s.pix_fmt = val.to_string(),
            "width" => s.width = val.parse().unwrap_or(0),
            "height" => s.height = val.parse().unwrap_or(0),
            "disposition.attached_pic" => s.cover = val == "1",
            _ => {}
        }
    }
    for s in streams.into_values() {
        match s.ty.as_str() {
            "audio" => p.has_audio = true,
            "video" if s.cover => p.has_cover = true,
            "video" if !p.has_video => {
                p.has_video = true;
                (p.width, p.height) = (s.width, s.height);
                (p.vcodec, p.pix_fmt) = (s.codec, s.pix_fmt);
            }
            _ => {}
        }
    }
    p
}

fn push(v: &mut Vec<OsString>, args: &[&str]) {
    v.extend(args.iter().map(OsString::from));
}

/// `scale` arguments capping the shorter side at `max` (0 = untouched) with both sides even,
/// which yuv420p encoders require.
fn scale_args(max: u32) -> String {
    if max == 0 {
        "w=trunc(iw/2)*2:h=trunc(ih/2)*2".into()
    } else {
        format!("w='if(gte(iw,ih),-2,min({max},trunc(iw/2)*2))':h='if(gte(iw,ih),min({max},trunc(ih/2)*2),-2)'")
    }
}

/// The codec family a video target is encoded with, or None for formats with a fixed CPU
/// encoder (GIF, AVI).
pub fn video_codec<'a>(target: &str, o: &'a MediaOptions) -> Option<&'a str> {
    match target {
        "webm" => Some("vp9"),
        "mp4" | "mkv" | "mov" => Some(o.codec.as_str()),
        _ => None,
    }
}

/// `accel` applies only to targets with a `video_codec`; pass `Accel::Cpu` otherwise.
pub fn ffmpeg_argv(ffmpeg: &Path, src: &Path, dst: &Path, kind: Kind, target: &str, o: &MediaOptions, p: &Probe, accel: &Accel) -> Result<Vec<OsString>, String> {
    let mut v: Vec<OsString> = vec![ffmpeg.into()];
    push(&mut v, &["-hide_banner", "-nostdin", "-y", "-loglevel", "error", "-progress", "pipe:1", "-nostats"]);
    let kbps = format!("{}k", o.audio_kbps.clamp(32, 320));
    let codec = video_codec(target, o).filter(|_| !formats::is_audio_target(target));
    // Decoding on the GPU keeps frames there for scaling and encoding; streams the decoder
    // cannot take are decoded in software and uploaded instead.
    let gpu_frames = gpu::hw_decodable(&p.vcodec, &p.pix_fmt);
    match (accel, codec) {
        (Accel::Nvenc, Some(_)) if gpu_frames => push(&mut v, &["-hwaccel", "cuda", "-hwaccel_output_format", "cuda"]),
        (Accel::Vaapi(dev), Some(_)) => {
            if gpu_frames {
                push(&mut v, &["-hwaccel", "vaapi", "-hwaccel_device"]);
                v.push(dev.into());
                push(&mut v, &["-hwaccel_output_format", "vaapi"]);
            } else {
                v.push("-vaapi_device".into());
                v.push(dev.into());
            }
        }
        _ => {}
    }
    v.push("-i".into());
    v.push(src.into());

    if formats::is_audio_target(target) {
        if !p.has_audio {
            return Err(tr("the file has no audio track").into());
        }
        push(&mut v, &["-map", "0:a:0"]);
        // Keep album art when both sides can carry it.
        if kind == Kind::Audio && p.has_cover && matches!(target, "mp3" | "flac" | "m4a") {
            push(&mut v, &["-map", "0:v:0", "-c:v", "copy", "-disposition:v:0", "attached_pic"]);
        } else {
            v.push("-vn".into());
        }
        match target {
            "mp3" => push(&mut v, &["-c:a", "libmp3lame", "-b:a", &kbps]),
            "ogg" => push(&mut v, &["-c:a", "libvorbis", "-b:a", &kbps]),
            "opus" => push(&mut v, &["-c:a", "libopus", "-b:a", &kbps]),
            "m4a" => push(&mut v, &["-c:a", "aac", "-b:a", &kbps]),
            "flac" => push(&mut v, &["-c:a", "flac"]),
            _ => {} // wav/aiff: ffmpeg's default PCM for the container
        }
        v.push(dst.into());
        return Ok(v);
    }

    if !p.has_video {
        return Err(tr("the file has no video track").into());
    }
    let crf = o.crf.clamp(0, 51);
    let scale = scale_args(o.max_height);
    match (target, codec, accel) {
        ("gif", ..) => {
            let max = if o.max_height == 0 { 480 } else { o.max_height.min(480) };
            let vf = format!(
                "[0:v:0]fps=15,scale={}:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4[out]",
                scale_args(max)
            );
            push(&mut v, &["-filter_complex", &vf, "-map", "[out]", "-loop", "0"]);
        }
        ("avi", ..) => {
            let q = ((crf as i32 - 14) / 3).clamp(2, 12).to_string();
            push(&mut v, &["-map", "0:v:0", "-map", "0:a?", "-vf", &format!("scale={scale}"), "-pix_fmt", "yuv420p"]);
            push(&mut v, &["-c:v", "mpeg4", "-q:v", &q, "-c:a", "libmp3lame", "-b:a", &kbps]);
        }
        (_, Some(c), Accel::Nvenc) => {
            let vf = if gpu_frames { format!("scale_cuda={scale}:format=yuv420p") } else { format!("scale={scale},format=yuv420p") };
            let preset = match o.speed.as_str() { "fast" => "p2", "slow" => "p6", _ => "p4" };
            let cq = match c { "h265" => crf + 4, "av1" => crf + 10, _ => crf }.min(51);
            push(&mut v, &["-map", "0:v:0", "-map", "0:a?", "-vf", &vf]);
            push(&mut v, &["-c:v", gpu::nvenc_encoder(c), "-preset", preset, "-tune", "hq", "-rc", "vbr", "-cq", &cq.to_string(), "-b:v", "0"]);
        }
        (_, Some(c), Accel::Vaapi(_)) => {
            let vf = if gpu_frames { format!("scale_vaapi={scale}:format=nv12") } else { format!("scale={scale},format=nv12,hwupload") };
            // h264/hevc take a QP; vp9 and av1 a 0–255 quantiser index.
            let q = match c { "h265" => (crf + 3).min(51), "vp9" | "av1" => ((crf + 8) * 4).min(255), _ => crf };
            push(&mut v, &["-map", "0:v:0", "-map", "0:a?", "-vf", &vf]);
            push(&mut v, &["-c:v", gpu::vaapi_encoder(c), "-rc_mode", "CQP", "-global_quality", &q.to_string()]);
        }
        (_, c, _) => {
            push(&mut v, &["-map", "0:v:0", "-map", "0:a?", "-vf", &format!("scale={scale}"), "-pix_fmt", "yuv420p"]);
            let preset = match o.speed.as_str() { "fast" => "veryfast", "slow" => "slow", _ => "medium" };
            match c.unwrap_or("h264") {
                "vp9" => {
                    let cpu = match o.speed.as_str() { "fast" => "5", "slow" => "1", _ => "3" };
                    push(&mut v, &["-c:v", "libvpx-vp9", "-crf", &(crf + 8).min(63).to_string(), "-b:v", "0", "-row-mt", "1", "-deadline", "good", "-cpu-used", cpu]);
                }
                "h265" => push(&mut v, &["-c:v", "libx265", "-crf", &(crf + 5).min(51).to_string(), "-preset", preset, "-x265-params", "log-level=error"]),
                "av1" => {
                    let p = match o.speed.as_str() { "fast" => "10", "slow" => "5", _ => "8" };
                    push(&mut v, &["-c:v", "libsvtav1", "-crf", &(crf + 12).min(63).to_string(), "-preset", p]);
                }
                _ => push(&mut v, &["-c:v", "libx264", "-crf", &crf.to_string(), "-preset", preset]),
            }
        }
    }
    match target {
        "webm" => push(&mut v, &["-c:a", "libopus", "-b:a", &kbps]),
        "mkv" => push(&mut v, &["-c:a", "aac", "-b:a", &kbps, "-map", "0:s?", "-c:s", "copy"]),
        "mp4" | "mov" => {
            push(&mut v, &["-c:a", "aac", "-b:a", &kbps, "-movflags", "+faststart"]);
            if codec == Some("h265") {
                // Apple players only accept HEVC tagged hvc1.
                push(&mut v, &["-tag:v", "hvc1"]);
            }
        }
        _ => {}
    }
    v.push(dst.into());
    Ok(v)
}

/// A frame for the file card: cover art for audio, a frame a second in for video.
pub fn thumbnail_argv(ffmpeg: &Path, src: &Path, dst: &Path, size: u32, p: &Probe) -> Vec<OsString> {
    let mut v: Vec<OsString> = vec![ffmpeg.into()];
    push(&mut v, &["-hide_banner", "-nostdin", "-y", "-loglevel", "error"]);
    if p.has_video && p.duration.is_some_and(|d| d > 2.0) {
        push(&mut v, &["-ss", "1"]);
    }
    v.push("-i".into());
    v.push(src.into());
    push(&mut v, &["-map", "0:v:0", "-frames:v", "1", "-vf", &format!("scale={size}:{size}:force_original_aspect_ratio=decrease")]);
    v.push(dst.into());
    v
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Progress {
    pub seconds: f64,
    pub speed: Option<f64>,
}

/// Feeds one `-progress` line; returns true when a progress block is complete.
pub fn progress_line(line: &str, p: &mut Progress) -> bool {
    let Some((k, v)) = line.trim().split_once('=') else { return false };
    match k {
        "out_time_us" => {
            if let Ok(us) = v.parse::<i64>() {
                p.seconds = us.max(0) as f64 / 1e6;
            }
        }
        "speed" => p.speed = v.trim_end_matches('x').trim().parse().ok().filter(|s: &f64| *s > 0.0),
        "progress" => return true,
        _ => {}
    }
    false
}

pub fn clock(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_flat() {
        let t = "streams.stream.0.codec_type=\"audio\"\nstreams.stream.0.disposition.attached_pic=0\n\
                 streams.stream.1.codec_type=\"video\"\nstreams.stream.1.width=500\nstreams.stream.1.height=500\n\
                 streams.stream.1.disposition.attached_pic=1\nformat.duration=\"184.5\"\n";
        let p = parse_probe(t);
        assert!(p.has_audio && p.has_cover && !p.has_video);
        assert_eq!(p.duration, Some(184.5));
        let p = parse_probe("streams.stream.0.codec_type=\"video\"\nstreams.stream.0.codec_name=\"h264\"\nstreams.stream.0.pix_fmt=\"yuv420p\"\nstreams.stream.0.width=1280\nstreams.stream.0.height=720\nformat.duration=\"N/A\"\n");
        assert!(p.has_video && p.duration.is_none());
        assert_eq!((p.vcodec.as_str(), p.pix_fmt.as_str()), ("h264", "yuv420p"));
        assert_eq!((p.width, p.height), (1280, 720));
    }

    #[test]
    fn progress_blocks() {
        let mut p = Progress::default();
        assert!(!progress_line("out_time_us=1500000", &mut p));
        assert!(!progress_line("speed=2.5x", &mut p));
        assert!(progress_line("progress=continue", &mut p));
        assert_eq!(p.seconds, 1.5);
        assert_eq!(p.speed, Some(2.5));
        progress_line("speed=N/A", &mut p);
        assert_eq!(p.speed, None);
    }

    #[test]
    fn clocks() {
        assert_eq!(clock(65.4), "1:05");
        assert_eq!(clock(3725.0), "1:02:05");
    }
}
