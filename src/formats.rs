//! Which extensions Morph reads and writes, grouped by kind.

use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Image,
}

const IMAGE_IN: &[&str] = &[
    "png", "jpg", "jpeg", "jpe", "jfif", "webp", "avif", "heic", "heif", "jxl", "gif", "tif", "tiff", "bmp", "svg",
    "ico", "tga", "psd", "jp2", "j2k", "exr", "hdr", "ppm", "pgm", "pbm", "pnm", "dng", "cr2", "cr3", "nef", "arw",
    "orf", "rw2", "raf",
];

const IMAGE_OUT: &[&str] = &["png", "jpg", "webp", "avif", "heic", "jxl", "gif", "tiff", "bmp"];

/// Formats that can hold several frames; converting between them keeps the animation.
const ANIMATED: &[&str] = &["gif", "webp"];

pub fn ext_of(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase()
}

pub fn kind_of(ext: &str) -> Option<Kind> {
    IMAGE_IN.contains(&ext).then_some(Kind::Image)
}

pub fn targets(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Image => IMAGE_OUT,
    }
}

pub fn all_inputs() -> impl Iterator<Item = &'static str> {
    IMAGE_IN.iter().copied()
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
        e => e,
    }
}

pub fn label(ext: &str) -> String {
    ext.to_uppercase()
}
