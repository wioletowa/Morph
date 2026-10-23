//! Locating the external converters. A copy shipped next to the binary wins over the system one.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct Tools {
    pub vips: Option<PathBuf>,
}

impl Tools {
    pub fn detect() -> Self {
        Tools { vips: find("vips") }
    }
}

fn find(name: &str) -> Option<PathBuf> {
    let bundled = std::env::current_exe().ok().and_then(|exe| {
        let dir = exe.parent()?.to_path_buf();
        [dir.join(name), dir.join("../lib/morph").join(name)].into_iter().find(|p| is_exe(p))
    });
    bundled.or_else(|| {
        std::env::var_os("PATH")
            .into_iter()
            .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
            .map(|d| d.join(name))
            .find(|p| is_exe(p))
    })
}

fn is_exe(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}
