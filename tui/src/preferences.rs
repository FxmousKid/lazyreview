//! Canonical settings with read-only compatibility for pre-rename installs.
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
        })
}
pub fn write_path() -> PathBuf {
    root().join("lazyreview/config.yml")
}
fn source_at(root: &Path) -> PathBuf {
    let current = root.join("lazyreview/config.yml");
    let legacy = root.join("review-workbench/config.yml");
    if !current.exists() && legacy.exists() {
        legacy
    } else {
        current
    }
}
pub fn read_path() -> PathBuf {
    source_at(&root())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_settings_win_and_legacy_settings_are_never_moved() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let current = root.join("lazyreview/config.yml");
        let legacy = root.join("review-workbench/config.yml");
        assert_eq!(source_at(root), current);
        std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        std::fs::write(
            &legacy,
            "gui:\n  themePreset: blue\nai:\n  profile: existing\n",
        )
        .unwrap();
        assert_eq!(source_at(root), legacy);
        std::fs::create_dir_all(current.parent().unwrap()).unwrap();
        std::fs::write(&current, "gui:\n  themePreset: custom\n").unwrap();
        assert_eq!(source_at(root), current);
        assert!(
            std::fs::read_to_string(legacy)
                .unwrap()
                .contains("existing")
        );
    }
}
