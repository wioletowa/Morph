//! Plain `key=value` config in $XDG_CONFIG_HOME/morph/config.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub lang: String,
    /// None means "next to the original file".
    pub output_dir: Option<PathBuf>,
    pub image_quality: u32,
    pub keep_metadata: bool,
    pub jobs: u32,
    pub image_target: String,
    pub width: i32,
    pub height: i32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            lang: "auto".into(),
            output_dir: None,
            image_quality: 90,
            keep_metadata: true,
            jobs: default_jobs(),
            image_target: "webp".into(),
            width: 760,
            height: 620,
        }
    }
}

pub fn default_jobs() -> u32 {
    let cores = std::thread::available_parallelism().map_or(2, |n| n.get() as u32);
    (cores / 2).clamp(1, 8)
}

fn dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_default()
        .join("morph")
}

pub fn cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .unwrap_or_else(std::env::temp_dir)
        .join("morph")
}

impl Config {
    pub fn load() -> Self {
        let mut c = Config::default();
        let Ok(text) = std::fs::read_to_string(dir().join("config")) else { return c };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim();
            match k.trim() {
                "lang" => c.lang = v.into(),
                "output_dir" => c.output_dir = (!v.is_empty()).then(|| PathBuf::from(v)),
                "image_quality" => c.image_quality = v.parse().unwrap_or(c.image_quality).clamp(1, 100),
                "keep_metadata" => c.keep_metadata = v == "true",
                "jobs" => c.jobs = v.parse().unwrap_or(c.jobs).clamp(1, 32),
                "image_target" => c.image_target = v.into(),
                "width" => c.width = v.parse().unwrap_or(c.width),
                "height" => c.height = v.parse().unwrap_or(c.height),
                _ => {}
            }
        }
        c
    }

    pub fn save(&self) {
        let out = self.output_dir.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        let text = format!(
            "lang={}\noutput_dir={out}\nimage_quality={}\nkeep_metadata={}\njobs={}\nimage_target={}\nwidth={}\nheight={}\n",
            self.lang, self.image_quality, self.keep_metadata, self.jobs, self.image_target, self.width, self.height,
        );
        let d = dir();
        if std::fs::create_dir_all(&d).is_ok() {
            let _ = std::fs::write(d.join("config"), text);
        }
    }
}
