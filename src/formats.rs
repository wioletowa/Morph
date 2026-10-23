//! Which extensions Morph reads and writes, grouped by kind.

use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Image,
    Audio,
    Video,
}

const IMAGE_IN: &[&str] = &[
    "png", "jpg", "jpeg", "jpe", "jfif", "webp", "avif", "heic", "heif", "jxl", "gif", "tif", "tiff", "bmp", "svg",
    "ico", "tga", "psd", "jp2", "j2k", "exr", "hdr", "ppm", "pgm", "pbm", "pnm", "dng", "cr2", "cr3", "nef", "arw",
    "orf", "rw2", "raf",
];
const AUDIO_IN: &[&str] = &[
    "mp3", "flac", "ogg", "oga", "opus", "wav", "m4a", "aac", "aiff", "aif", "wma", "ape", "wv", "amr", "mka", "ac3",
];
const VIDEO_IN: &[&str] = &[
    "mp4", "mkv", "webm", "mov", "avi", "m4v", "wmv", "flv", "mpg", "mpeg", "ts", "m2ts", "mts", "3gp", "ogv",
];

const IMAGE_OUT: &[&str] = &["png", "jpg", "webp", "avif", "heic", "jxl", "gif", "tiff", "bmp"];
const AUDIO_OUT: &[&str] = &["mp3", "flac", "ogg", "opus", "wav", "m4a", "aiff"];
const VIDEO_OUT: &[&str] = &["mp4", "mkv", "webm", "mov", "avi", "gif"];
/// A video can also become any audio format (the soundtrack is extracted).
const VIDEO_TARGETS: &[&str] = &["mp4", "mkv", "webm", "mov", "avi", "gif", "mp3", "flac", "ogg", "opus", "wav", "m4a", "aiff"];
/// GIF animations can turn into short videos too.
const GIF_TARGETS: &[&str] = &["png", "jpg", "webp", "avif", "heic", "jxl", "gif", "tiff", "bmp", "mp4", "webm"];

/// Formats that can hold several frames; converting between them keeps the animation.
const ANIMATED: &[&str] = &["gif", "webp"];

pub fn ext_of(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase()
}

pub fn kind_of(ext: &str) -> Option<Kind> {
    if IMAGE_IN.contains(&ext) {
        Some(Kind::Image)
    } else if AUDIO_IN.contains(&ext) {
        Some(Kind::Audio)
    } else if VIDEO_IN.contains(&ext) {
        Some(Kind::Video)
    } else {
        None
    }
}

/// What a file with extension `src` of kind `kind` can be converted to.
pub fn targets(kind: Kind, src: &str) -> &'static [&'static str] {
    match kind {
        Kind::Image if src == "gif" => GIF_TARGETS,
        Kind::Image => IMAGE_OUT,
        Kind::Audio => AUDIO_OUT,
        Kind::Video => VIDEO_TARGETS,
    }
}

/// Targets offered in "Set all to", in display order.
pub fn all_targets() -> impl Iterator<Item = (Kind, &'static str)> {
    let img = IMAGE_OUT.iter().map(|t| (Kind::Image, *t));
    let aud = AUDIO_OUT.iter().map(|t| (Kind::Audio, *t));
    let vid = VIDEO_OUT.iter().filter(|t| **t != "gif").map(|t| (Kind::Video, *t));
    img.chain(vid).chain(aud)
}

pub fn all_inputs() -> impl Iterator<Item = &'static str> {
    IMAGE_IN.iter().chain(AUDIO_IN).chain(VIDEO_IN).copied()
}

pub fn is_audio_target(ext: &str) -> bool {
    AUDIO_OUT.contains(&ext)
}

/// Whether vips handles this conversion; everything else goes through ffmpeg.
pub fn uses_vips(kind: Kind, target: &str) -> bool {
    kind == Kind::Image && IMAGE_OUT.contains(&target)
}

pub fn keeps_frames(src: &str, dst: &str) -> bool {
    ANIMATED.contains(&src) && ANIMATED.contains(&dst)
}

/// Collapses aliases so "jpeg" → "jpg" and "tif" → "tiff" compare equal to targets.
pub fn canonical(ext: &str) -> &str {
    match ext {
        "jpeg" | "jpe" | "jfif" => "jpg",
        "tif" => "tiff",
        "heif" => "heic",
        "aif" => "aiff",
        "oga" => "ogg",
        e => e,
    }
}

pub fn label(ext: &str) -> String {
    ext.to_uppercase()
}
