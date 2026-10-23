//! Hardware video encoders. An encoder counts only if a one-frame test encode succeeds:
//! ffmpeg lists e.g. `av1_nvenc` even on GPUs that cannot run it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Codec families Morph encodes video with.
pub const CODECS: [&str; 4] = ["h264", "h265", "av1", "vp9"];

#[derive(Clone, Debug, PartialEq)]
pub enum Accel {
    Cpu,
    Nvenc,
    Vaapi(PathBuf),
}

impl Accel {
    pub fn label(&self) -> &'static str {
        match self {
            Accel::Cpu => "CPU",
            Accel::Nvenc => "NVENC",
            Accel::Vaapi(_) => "VAAPI",
        }
    }
}

#[derive(Clone, Debug)]
pub struct VaapiDevice {
    pub path: PathBuf,
    pub vendor: &'static str,
    pub codecs: Vec<&'static str>,
}

#[derive(Clone, Debug, Default)]
pub struct GpuInfo {
    pub nvidia_name: Option<String>,
    pub nvenc: Vec<&'static str>,
    pub vaapi: Vec<VaapiDevice>,
}

impl GpuInfo {
    /// `mode` is "auto", "nvidia", "vaapi" or "cpu"; `device` an empty string or a render node path.
    pub fn choose(&self, mode: &str, device: &str, codec: &str) -> Accel {
        let nvenc = || self.nvenc.contains(&codec).then_some(Accel::Nvenc);
        let vaapi = || {
            self.vaapi
                .iter()
                .filter(|d| device.is_empty() || d.path == Path::new(device))
                .find(|d| d.codecs.contains(&codec))
                .map(|d| Accel::Vaapi(d.path.clone()))
        };
        match mode {
            "cpu" => None,
            "nvidia" => nvenc(),
            "vaapi" => vaapi(),
            _ => nvenc().or_else(vaapi),
        }
        .unwrap_or(Accel::Cpu)
    }
}

pub fn nvenc_encoder(codec: &str) -> &'static str {
    match codec {
        "h265" => "hevc_nvenc",
        "av1" => "av1_nvenc",
        _ => "h264_nvenc",
    }
}

pub fn vaapi_encoder(codec: &str) -> &'static str {
    match codec {
        "h265" => "hevc_vaapi",
        "av1" => "av1_vaapi",
        "vp9" => "vp9_vaapi",
        _ => "h264_vaapi",
    }
}

/// Render nodes with the vendor name of the GPU behind each.
pub fn render_nodes() -> Vec<(PathBuf, &'static str)> {
    let Ok(dir) = std::fs::read_dir("/dev/dri") else { return Vec::new() };
    let mut nodes: Vec<_> = dir
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("renderD")))
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            let vendor = std::fs::read_to_string(format!("/sys/class/drm/{name}/device/vendor")).unwrap_or_default();
            let vendor = match vendor.trim() {
                "0x8086" => "Intel",
                "0x1002" => "AMD",
                "0x10de" => "NVIDIA",
                _ => "GPU",
            };
            (p, vendor)
        })
        .collect();
    nodes.sort();
    nodes
}

/// The model name the NVIDIA driver reports, e.g. "Quadro T2000".
pub fn nvidia_name() -> Option<String> {
    let dir = std::fs::read_dir("/proc/driver/nvidia/gpus").ok()?;
    dir.filter_map(|e| e.ok()).find_map(|e| {
        let info = std::fs::read_to_string(e.path().join("information")).ok()?;
        info.lines().find_map(|l| l.strip_prefix("Model:")).map(|m| m.trim().to_string())
    })
}

/// Encoder names from `ffmpeg -encoders`.
pub fn parse_encoders(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let flags = it.next()?;
            (flags.len() == 6 && flags.starts_with('V')).then(|| it.next().map(String::from))?
        })
        .collect()
}

fn base(ffmpeg: &Path) -> Vec<OsString> {
    vec![ffmpeg.into(), "-hide_banner".into(), "-nostdin".into(), "-loglevel".into(), "error".into()]
}

fn push(v: &mut Vec<OsString>, args: &[&str]) {
    v.extend(args.iter().map(OsString::from));
}

pub fn nvenc_test_argv(ffmpeg: &Path, codec: &str) -> Vec<OsString> {
    let mut v = base(ffmpeg);
    push(&mut v, &["-f", "lavfi", "-i", "nullsrc=s=256x256:d=0.1", "-frames:v", "1", "-c:v", nvenc_encoder(codec), "-f", "null", "-"]);
    v
}

pub fn vaapi_test_argv(ffmpeg: &Path, device: &Path, codec: &str) -> Vec<OsString> {
    let mut v = base(ffmpeg);
    v.push("-vaapi_device".into());
    v.push(device.into());
    push(&mut v, &["-f", "lavfi", "-i", "nullsrc=s=256x256:d=0.1", "-vf", "format=nv12,hwupload", "-frames:v", "1", "-c:v", vaapi_encoder(codec), "-f", "null", "-"]);
    v
}

/// Whether the GPU decoder can take this stream and hand frames straight to GPU filters.
/// Anything else is decoded in software and uploaded, which every encoder accepts.
pub fn hw_decodable(vcodec: &str, pix_fmt: &str) -> bool {
    matches!(vcodec, "h264" | "hevc" | "vp9" | "vp8" | "av1" | "mpeg2video" | "mpeg1video" | "vc1")
        && matches!(pix_fmt, "yuv420p" | "yuvj420p" | "nv12" | "yuv420p10le" | "p010le")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> GpuInfo {
        GpuInfo {
            nvidia_name: Some("T2000".into()),
            nvenc: vec!["h264", "h265"],
            vaapi: vec![VaapiDevice { path: "/dev/dri/renderD129".into(), vendor: "Intel", codecs: vec!["h264", "vp9"] }],
        }
    }

    #[test]
    fn choice_follows_mode_and_support() {
        let g = info();
        assert_eq!(g.choose("auto", "", "h264"), Accel::Nvenc);
        assert_eq!(g.choose("auto", "", "vp9"), Accel::Vaapi("/dev/dri/renderD129".into()));
        assert_eq!(g.choose("auto", "", "av1"), Accel::Cpu);
        assert_eq!(g.choose("vaapi", "", "h264"), Accel::Vaapi("/dev/dri/renderD129".into()));
        assert_eq!(g.choose("vaapi", "/dev/dri/renderD128", "h264"), Accel::Cpu);
        assert_eq!(g.choose("nvidia", "", "vp9"), Accel::Cpu);
        assert_eq!(g.choose("cpu", "", "h264"), Accel::Cpu);
    }

    #[test]
    fn encoder_list() {
        let t = " V....D h264_nvenc           NVIDIA NVENC H.264 encoder (codec h264)\n A....D aac   AAC\n V..... h264_qsv  QSV\n";
        assert_eq!(parse_encoders(t), vec!["h264_nvenc", "h264_qsv"]);
    }
}
