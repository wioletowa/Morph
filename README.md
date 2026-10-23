# Morph

A native file converter for the GNOME desktop. Drop files in, pick a format, convert — on your own
computer, with nothing uploaded. Built with Rust, GTK 4 and libadwaita.

- **Images** — PNG, JPEG, WebP, AVIF, HEIC, JPEG XL, GIF, TIFF, BMP. Also reads SVG, PSD, EXR and
  camera RAW. Applies EXIF rotation, keeps GIF/WebP animation, can strip metadata.
- **Audio** — MP3, FLAC, Ogg, Opus, WAV, M4A, AIFF. Keeps tags and album art.
- **Video** — MP4, MKV, WebM, MOV, AVI, GIF, or any audio format to extract the soundtrack.
  H.264, H.265 or AV1; keeps subtitles in MKV.
- **GPU encoding** — NVIDIA NVENC or VAAPI (Intel, AMD). Morph test-encodes one frame with each
  encoder at startup and only uses the ones that work; anything the GPU cannot do, or a GPU encode
  that fails, runs on the CPU instead.

## Requirements

- GTK ≥ 4.18 and libadwaita ≥ 1.7
- `vips` (libvips) for images
- `ffmpeg` and `ffprobe` for audio and video
- Optional, for GPU encoding: the NVIDIA driver (NVENC), or a VA-API driver such as
  `intel-media-driver` or Mesa's `libva-mesa-driver`

Morph looks for `vips`, `ffmpeg` and `ffprobe` next to its own binary (or in `../lib/morph`)
first, then in `PATH`, so a build can ship its own copies.

## Building

```sh
cargo build --release
./target/release/morph
```

On Arch Linux, `makepkg -si` in this directory builds and installs the package from `PKGBUILD`.

## Credits

Morph is inspired by [VERT](https://vert.sh), the open-source file converter. It is an independent
reimplementation of the idea: it contains no code, artwork or text from VERT and is not affiliated
with the VERT authors.

Conversions are done by external programs that Morph runs as separate processes; they are not
linked into Morph and keep their own licenses:

- [libvips](https://www.libvips.org) — LGPL-2.1-or-later
- [FFmpeg](https://ffmpeg.org) — LGPL-2.1-or-later, or GPL depending on how it was built

## License

[MIT](LICENSE)
