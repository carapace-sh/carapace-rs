//! Filename styling based on file features (port of elvish's `lscolors`
//! as vendored by Go carapace): a reverse-engineered implementation of
//! the `LS_COLORS` environment variable used by GNU coreutils.

use std::collections::HashMap;
use std::path::Path;

use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};

/// The GNU coreutils default when `LS_COLORS` is unset or empty.
pub const DEFAULT_LS_COLORS: &str = "rs=:di=01;34:ln=01;36:mh=:pi=40;33:so=01;35:do=01;35:bd=40;33;01:cd=40;33;01:or=40;31;01:su=37;41:sg=30;43:ca=30;41:tw=30;42:ow=34;42:st=37;44:ex=01;32:*.tar=01;31:*.tgz=01;31:*.arc=01;31:*.arj=01;31:*.taz=01;31:*.lha=01;31:*.lz4=01;31:*.lzh=01;31:*.lzma=01;31:*.tlz=01;31:*.txz=01;31:*.tzo=01;31:*.t7z=01;31:*.zip=01;31:*.z=01;31:*.Z=01;31:*.dz=01;31:*.gz=01;31:*.lrz=01;31:*.lz=01;31:*.lzo=01;31:*.xz=01;31:*.bz2=01;31:*.bz=01;31:*.tbz=01;31:*.tbz2=01;31:*.tz=01;31:*.deb=01;31:*.rpm=01;31:*.jar=01;31:*.war=01;31:*.ear=01;31:*.sar=01;31:*.rar=01;31:*.alz=01;31:*.ace=01;31:*.zoo=01;31:*.cpio=01;31:*.7z=01;31:*.rz=01;31:*.cab=01;31:*.jpg=01;35:*.jpeg=01;35:*.gif=01;35:*.bmp=01;35:*.pbm=01;35:*.pgm=01;35:*.ppm=01;35:*.tga=01;35:*.xbm=01;35:*.xpm=01;35:*.tif=01;35:*.tiff=01;35:*.png=01;35:*.svg=01;35:*.svgz=01;35:*.mng=01;35:*.pcx=01;35:*.mov=01;35:*.mpg=01;35:*.mpeg=01;35:*.m2v=01;35:*.mkv=01;35:*.webm=01;35:*.ogm=01;35:*.mp4=01;35:*.m4v=01;35:*.mp4v=01;35:*.vob=01;35:*.qt=01;35:*.nuv=01;35:*.wmv=01;35:*.asf=01;35:*.rm=01;35:*.rmvb=01;35:*.flc=01;35:*.avi=01;35:*.fli=01;35:*.flv=01;35:*.gl=01;35:*.dl=01;35:*.xcf=01;35:*.xwd=01;35:*.yuv=01;35:*.cgm=01;35:*.emf=01;35:*.axv=01;35:*.anx=01;35:*.ogv=01;35:*.ogx=01;35:*.aac=36:*.au=36:*.flac=36:*.mid=36:*.midi=36:*.mka=36:*.mp3=36:*.mpc=36:*.ogg=36:*.ra=36:*.wav=36:*.axa=36:*.oga=36:*.spx=36:*.xspf=36:";

/// A file feature recognized by LS_COLORS (Go `feature`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Feature {
    Regular,
    Directory,
    Symlink,
    MultiHardLink,
    NamedPipe,
    Socket,
    Door,
    BlockDevice,
    CharDevice,
    OrphanedSymlink,
    Setuid,
    Setgid,
    Executable,
    WorldWritableStickyDirectory,
    WorldWritableDirectory,
    StickyDirectory,
}

fn feature_for_name(name: &str) -> Option<Feature> {
    Some(match name {
        "rs" => Feature::Regular,
        "di" => Feature::Directory,
        "ln" => Feature::Symlink,
        "mh" => Feature::MultiHardLink,
        "pi" => Feature::NamedPipe,
        "so" => Feature::Socket,
        "do" => Feature::Door,
        "bd" => Feature::BlockDevice,
        "cd" => Feature::CharDevice,
        "or" => Feature::OrphanedSymlink,
        "su" => Feature::Setuid,
        "sg" => Feature::Setgid,
        // "ca" (capability) is unsupported, like the Go original
        "tw" => Feature::WorldWritableStickyDirectory,
        "ow" => Feature::WorldWritableDirectory,
        "st" => Feature::StickyDirectory,
        "ex" => Feature::Executable,
        _ => return None,
    })
}

/// Styles filenames based on file features (Go `Colorist`).
pub struct Colorist {
    style_for_feature: HashMap<Feature, String>,
    style_for_ext: HashMap<String, String>,
}

/// Build a colorist from an `LS_COLORS` string; the GNU default applies
/// when it is empty (Go `GetColorist`).
pub fn colorist(ls_colors: &str) -> Colorist {
    let source = if ls_colors.is_empty() {
        DEFAULT_LS_COLORS
    } else {
        ls_colors
    };
    parse_ls_color(source)
}

/// Parse a string in the LS_COLORS format; erroneous fields are silently
/// ignored (Go `parseLsColor`).
fn parse_ls_color(source: &str) -> Colorist {
    let mut colorist = Colorist {
        style_for_feature: HashMap::new(),
        style_for_ext: HashMap::new(),
    };
    for spec in source.split(':') {
        let Some((key, value)) = spec.split_once('=') else {
            continue;
        };
        // segments of all zeroes carry no style
        let filtered: Vec<&str> = value
            .split(';')
            .filter(|segment| !segment.chars().all(|c| c == '0'))
            .collect();
        if filtered.is_empty() {
            continue;
        }
        let value = filtered.join(";");
        if let Some(ext) = key.strip_prefix("*.") {
            colorist.style_for_ext.insert(format!(".{ext}"), value);
        } else if let Some(feature) = feature_for_name(key) {
            colorist.style_for_feature.insert(feature, value);
        }
    }
    colorist
}

impl Colorist {
    /// The style for the named file, by feature first and extension for
    /// regular files (Go `GetStyle`).
    pub fn get_style(&self, fname: &str) -> String {
        let mh = self
            .style_for_feature
            .get(&Feature::MultiHardLink)
            .is_some_and(|style| !style.trim_matches('0').is_empty());
        let feature = determine_feature(Path::new(fname), mh).unwrap_or(Feature::Regular);
        if feature == Feature::Regular {
            if let Some(ext) = path_ext(fname) {
                if let Some(style) = self.style_for_ext.get(&ext) {
                    return style.clone();
                }
            }
        }
        self.style_for_feature
            .get(&feature)
            .cloned()
            .unwrap_or_default()
    }

    /// The style for the named file by extension only, defaulting to the
    /// directory style (Go `GetStyleExt`).
    pub fn get_style_ext(&self, fname: &str) -> String {
        if !fname.ends_with('/') {
            if let Some(ext) = path_ext(fname) {
                if let Some(style) = self.style_for_ext.get(&ext) {
                    return style.clone();
                }
            }
        }
        self.style_for_feature
            .get(&Feature::Directory)
            .cloned()
            .unwrap_or_default()
    }
}

/// `path.Ext` analog: the extension including the leading dot, from the
/// last dot after the last slash.
fn path_ext(fname: &str) -> Option<String> {
    let filename = fname.rsplit('/').next().unwrap_or(fname);
    filename.rfind('.').and_then(|index| {
        let ext = &filename[index..];
        (!ext.is_empty()).then(|| ext.to_owned())
    })
}

const WORLD_WRITABLE: u32 = 0o002;

fn has_mode(mode: u32, bit: u32) -> bool {
    mode & bit == bit
}

/// The feature of the file at the given path (Go `determineFeature`).
fn determine_feature(fname: &Path, mh: bool) -> Option<Feature> {
    let metadata = std::fs::symlink_metadata(fname).ok()?;

    // symlinks have the highest precedence
    if metadata.file_type().is_symlink() {
        return if std::fs::metadata(fname).is_err() {
            Some(Feature::OrphanedSymlink)
        } else {
            Some(Feature::Symlink)
        };
    }

    if mh && metadata.nlink() > 1 {
        return Some(Feature::MultiHardLink);
    }

    let file_type = metadata.file_type();
    let mode = metadata.permissions().mode();
    if file_type.is_fifo() {
        return Some(Feature::NamedPipe);
    }
    if file_type.is_socket() {
        return Some(Feature::Socket);
    }
    if file_type.is_char_device() {
        return Some(Feature::CharDevice);
    }
    if file_type.is_block_device() {
        return Some(Feature::BlockDevice);
    }
    if file_type.is_dir() {
        let sticky = has_mode(mode, 0o1000);
        return Some(match (sticky, has_mode(mode, WORLD_WRITABLE)) {
            (true, true) => Feature::WorldWritableStickyDirectory,
            (false, true) => Feature::WorldWritableDirectory,
            (true, false) => Feature::StickyDirectory,
            (false, false) => Feature::Directory,
        });
    }

    if has_mode(mode, 0o4000) {
        return Some(Feature::Setuid);
    }
    if has_mode(mode, 0o2000) {
        return Some(Feature::Setgid);
    }
    if !metadata.is_dir() && has_mode(mode, 0o111) {
        return Some(Feature::Executable);
    }

    Some(Feature::Regular)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    /// Serializes filesystem-mutating tests.
    fn fs_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn parses_ls_colors() {
        let colorist = colorist("di=01;34:*.tar=01;31:*.jpg=01;35:invalid");
        assert_eq!(colorist.style_for_feature[&Feature::Directory], "01;34");
        assert_eq!(colorist.style_for_ext[".tar"], "01;31");
        assert_eq!(colorist.style_for_ext[".jpg"], "01;35");
        assert!(!colorist.style_for_feature.contains_key(&Feature::Regular));
    }

    #[test]
    fn filters_all_zero_segments() {
        // `00;31` keeps only the meaningful segment
        let colorist = colorist("*.log=00;31:di=01;00;34");
        assert_eq!(colorist.style_for_ext[".log"], "31");
        assert_eq!(colorist.style_for_feature[&Feature::Directory], "01;34");
    }

    #[test]
    fn default_colors_directories() {
        let colorist = colorist("");
        assert_eq!(colorist.get_style_ext("whatever"), "01;34");
    }

    #[test]
    fn styles_by_file_feature() {
        let _guard = fs_lock();
        let base = std::env::temp_dir().join(format!("carapace-lscolors-{}", std::process::id()));
        std::fs::create_dir_all(base.join("subdir")).unwrap();
        std::fs::write(base.join("plain.txt"), "").unwrap();
        std::fs::write(base.join("archive.tar"), "").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                base.join("plain.txt"),
                std::fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }

        let colorist = colorist("");
        assert_eq!(
            colorist.get_style(&base.join("subdir").display().to_string()),
            "01;34"
        );
        assert_eq!(
            colorist.get_style(&base.join("archive.tar").display().to_string()),
            "01;31"
        );
        assert_eq!(
            colorist.get_style(&base.join("plain.txt").display().to_string()),
            "01;32" // executable
        );

        std::fs::remove_dir_all(base).unwrap();
    }
}
