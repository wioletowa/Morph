//! Building converter command lines and picking output paths.

use crate::formats;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ImageOptions {
    pub quality: u32,
    pub keep_metadata: bool,
}

/// `vips autorot` applies the EXIF orientation, then writes `dst`; save options ride on the
/// filename. BMP has no native saver, so it goes through `magicksave`, which cannot autorotate.
pub fn image_argv(vips: &Path, src: &Path, dst: &Path, target: &str, o: &ImageOptions) -> Vec<OsString> {
    let src_ext = formats::canonical(&formats::ext_of(src)).to_string();
    let mut input = src.as_os_str().to_os_string();
    input.push(if formats::keeps_frames(&src_ext, target) { "[n=-1]" } else { "[]" });

    let keep = if o.keep_metadata { "all" } else { "icc" };
    if target == "bmp" {
        return [vips.as_os_str(), "magicksave".as_ref(), input.as_os_str(), dst.as_os_str(), "--format=bmp".as_ref()]
            .map(OsString::from)
            .to_vec();
    }
    let q = o.quality.clamp(1, 100);
    let opts = match target {
        "jpg" => format!("Q={q},keep={keep},background=65535"),
        "webp" | "avif" | "heic" | "jxl" => format!("Q={q},keep={keep}"),
        "tiff" => format!("compression=deflate,keep={keep}"),
        "png" => format!("keep={keep}"),
        _ => String::new(),
    };
    let mut output = dst.as_os_str().to_os_string();
    output.push(format!("[{opts}]"));
    vec![vips.into(), "autorot".into(), input, output]
}

pub fn thumbnail_argv(vips: &Path, src: &Path, dst: &Path, size: u32) -> Vec<OsString> {
    let s = size.to_string();
    [vips.as_os_str(), "thumbnail".as_ref(), src.as_os_str(), dst.as_os_str(), s.as_ref(), "--height".as_ref(), s.as_ref()]
        .map(OsString::from)
        .to_vec()
}

/// `stem.ext` in `dir`, or `stem (N).ext` when that is taken on disk or by `taken`.
pub fn output_path(dir: &Path, src: &Path, ext: &str, taken: &dyn Fn(&Path) -> bool) -> PathBuf {
    let stem = src.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
    let free = |p: &Path| p != src && !p.exists() && !taken(p);
    let first = dir.join(format!("{stem}.{ext}"));
    if free(&first) {
        return first;
    }
    (1..)
        .map(|n| dir.join(format!("{stem} ({n}).{ext}")))
        .find(|p| free(p))
        .unwrap()
}

/// Hidden sibling the converter writes into; renamed over the real name only on success.
pub fn temp_path(dst: &Path, id: u64) -> PathBuf {
    let ext = dst.extension().and_then(|e| e.to_str()).unwrap_or("");
    let name = dst.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    dst.with_file_name(format!(".{name}.morph-{}-{id}.{ext}", std::process::id()))
}

/// vips prints a warning for every optional module it cannot load; keep only real errors.
pub fn clean_stderr(s: &str) -> String {
    s.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.contains("-WARNING **"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 { format!("{bytes} B") } else { format!("{v:.1} {}", UNITS[u]) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_names_skip_taken_and_source() {
        let dir = std::env::temp_dir().join(format!("morph-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("a.png");
        std::fs::write(&src, b"x").unwrap();
        assert_eq!(output_path(&dir, &src, "webp", &|_| false), dir.join("a.webp"));
        assert_eq!(output_path(&dir, &src, "png", &|_| false), dir.join("a (1).png"));
        let taken = dir.join("a.webp");
        assert_eq!(output_path(&dir, &src, "webp", &|p| p == taken), dir.join("a (1).webp"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn stderr_drops_module_warnings() {
        let s = "\n(process:1): VIPS-WARNING **: 18:08: unable to load x.so\nVipsForeignLoad: bad file\n";
        assert_eq!(clean_stderr(s), "VipsForeignLoad: bad file");
    }
}
