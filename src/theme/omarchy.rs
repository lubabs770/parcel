//! Omarchy (omarchy.org) keeps the active theme in a "current theme" dir:
//! ~/.local/state/omarchy/current/theme (v4) or ~/.config/omarchy/current/theme (v3),
//! with colors.toml (older themes: only alacritty.toml).

use super::{Theme, rgb};
use crate::config::xdg;
use ratatui::style::Color;
use serde::Deserialize;
use std::{collections::HashMap, env, fs, path::{Path, PathBuf}};

const HOOK_LINE: &str = "pkill -USR1 -x parcel 2>/dev/null || true  # parcel: repaint on theme change";

pub fn theme_dir() -> Option<PathBuf> {
    if let Some(d) = env::var_os("PARCEL_OMARCHY_THEME") {
        let d = PathBuf::from(d);
        return has_files(&d).then_some(d);
    }
    [xdg("XDG_STATE_HOME", ".local/state"), xdg("XDG_CONFIG_HOME", ".config")]
        .into_iter()
        .map(|base| base.join("omarchy/current/theme"))
        .find(|d| has_files(d))
}

fn has_files(d: &Path) -> bool {
    ["colors.toml", "alacritty.toml"].iter().any(|f| d.join(f).is_file())
}

/// Human name of the current theme ("Tokyo Night").
pub fn name() -> Option<String> {
    let dir = theme_dir()?;
    fs::read_to_string(dir.join("../theme.name"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| fs::canonicalize(&dir).ok()?.file_name().map(|n| n.to_string_lossy().into_owned()))
}

/// Changes whenever the theme changes; a few stats, cheap enough to poll.
pub fn signature() -> String {
    theme_dir().map(|d| signature_of(&d)).unwrap_or_default()
}

pub fn signature_of(dir: &Path) -> String {
    let mut sig = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf()).display().to_string();
    for f in ["colors.toml", "alacritty.toml", "../theme.name"] {
        if let Ok(m) = fs::metadata(dir.join(f)) {
            sig += &format!("|{f}:{:?}:{}", m.modified().ok(), m.len());
        }
    }
    sig
}

/// Omarchy's colors.toml.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Colors {
    pub mode: String,
    pub accent: String,
    pub selection: String,
    pub muted: String,
    pub background: String,
    pub dark_background: String,
    pub lighter_background: String,
    pub foreground: String,
    pub dark_foreground: String,
    pub red: String,
    pub yellow: String,
    pub green: String,
    pub cyan: String,
    pub blue: String,
    pub magenta: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Alacritty {
    colors: AlacrittyColors,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct AlacrittyColors {
    primary: Primary,
    normal: HashMap<String, String>,
    bright: HashMap<String, String>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Primary {
    background: String,
    foreground: String,
}

pub fn load() -> Result<Theme, String> {
    load_dir(&theme_dir().ok_or("no Omarchy theme found")?)
}

pub fn load_dir(dir: &Path) -> Result<Theme, String> {
    let colors = fs::read_to_string(dir.join("colors.toml"))
        .ok()
        .and_then(|s| toml::from_str::<Colors>(&s).ok())
        .filter(|c| rgb(&c.background).is_some());
    let c = match colors {
        Some(c) => c,
        None => {
            let s = fs::read_to_string(dir.join("alacritty.toml")).map_err(|e| format!("Omarchy theme: {e}"))?;
            let a: Alacritty = toml::from_str(&s).map_err(|e| format!("Omarchy alacritty.toml: {e}"))?;
            let (n, b) = (&a.colors.normal, &a.colors.bright);
            let g = |m: &HashMap<String, String>, k: &str| m.get(k).cloned().unwrap_or_default();
            Colors {
                background: a.colors.primary.background,
                foreground: a.colors.primary.foreground,
                accent: g(n, "blue"),
                muted: g(b, "black"),
                red: g(n, "red"),
                yellow: g(n, "yellow"),
                green: g(n, "green"),
                cyan: g(n, "cyan"),
                blue: g(n, "blue"),
                magenta: g(n, "magenta"),
                ..Default::default()
            }
        }
    };
    Ok(map(&c))
}

type Rgb = (u8, u8, u8);

/// Map Omarchy's palette onto parcel's tokens; derive any layer it leaves out from the background.
pub fn map(c: &Colors) -> Theme {
    let bg = rgb(&c.background).unwrap_or((0x1a, 0x1b, 0x26));
    let fg = rgb(&c.foreground).unwrap_or((0xc0, 0xca, 0xf5));
    let light = c.mode.eq_ignore_ascii_case("light") || (c.mode.is_empty() && luminance(bg) > 0.5);
    let (deeper, raised) = if light {
        (mix(bg, (0, 0, 0), 0.04), mix(bg, (0, 0, 0), 0.10))
    } else {
        (mix(bg, (0, 0, 0), 0.20), mix(bg, (255, 255, 255), 0.08))
    };
    let pick = |vals: &[&String], def: Rgb| vals.iter().find_map(|v| rgb(v)).unwrap_or(def);
    let accent = pick(&[&c.accent, &c.blue], fg);
    let col = |(r, g, b): Rgb| Color::Rgb(r, g, b);
    Theme {
        bg: col(bg),
        surface: col(pick(&[&c.dark_background], deeper)),
        overlay: col(pick(&[&c.lighter_background], raised)),
        fg: col(fg),
        muted: col(pick(&[&c.muted, &c.dark_foreground], mix(fg, bg, 0.5))),
        accent: col(accent),
        unread: col(pick(&[&c.cyan], accent)),
        flagged: col(pick(&[&c.yellow], accent)),
        danger: col(pick(&[&c.red], accent)),
        ok: col(pick(&[&c.green], accent)),
        selection: col(pick(&[&c.selection], mix(bg, accent, 0.25))),
        border: col(pick(&[&c.lighter_background], raised)),
    }
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    (f(a.0, b.0), f(a.1, b.1), f(a.2, b.2))
}

fn luminance((r, g, b): Rgb) -> f32 {
    (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) / 255.0
}

pub fn hook_path() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("omarchy/hooks/theme-set")
}

pub fn hook_installed(path: &Path) -> bool {
    fs::read_to_string(path).is_ok_and(|s| s.contains("# parcel:"))
}

/// Append the repaint line to Omarchy's theme-set hook. Ok(false) if already there.
pub fn hook_install(path: &Path) -> Result<bool, String> {
    if hook_installed(path) {
        return Ok(false);
    }
    let mut s = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "#!/bin/sh\n".into(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s.push_str(HOOK_LINE);
    s.push('\n');
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::write(path, s).map_err(|e| format!("{}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }
    Ok(true)
}

/// Remove parcel's line. Ok(false) if it wasn't there.
pub fn hook_uninstall(path: &Path) -> Result<bool, String> {
    let Ok(s) = fs::read_to_string(path) else { return Ok(false) };
    if !s.contains("# parcel:") {
        return Ok(false);
    }
    let had_trailing_newline = s.ends_with('\n');
    let kept: String = s.lines().filter(|l| !l.contains("# parcel:")).collect::<Vec<_>>().join("\n");
    let kept = if had_trailing_newline && !kept.is_empty() {
        format!("{kept}\n")
    } else if kept.is_empty() {
        String::new()
    } else {
        kept
    };
    fs::write(path, kept).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb_of(c: Color) -> (u8, u8, u8) {
        let Color::Rgb(r, g, b) = c else { panic!("not rgb") };
        (r, g, b)
    }

    #[test]
    fn maps_colors_and_derives_missing_layers() {
        let c = Colors { background: "#1a1b26".into(), foreground: "#c0caf5".into(), accent: "#7aa2f7".into(), red: "#f7768e".into(), ..Default::default() };
        let t = map(&c);
        assert_eq!(rgb_of(t.bg), (0x1a, 0x1b, 0x26));
        assert_eq!(rgb_of(t.accent), (0x7a, 0xa2, 0xf7));
        assert_eq!(rgb_of(t.danger), (0xf7, 0x76, 0x8e));
        assert_eq!(t.flagged, t.accent, "no yellow → accent");
        assert!(rgb_of(t.surface).0 < 0x1a, "dark: surface is deeper than bg");
    }

    #[test]
    fn light_mode_from_luminance() {
        let t = map(&Colors { background: "#eff1f5".into(), foreground: "#4c4f69".into(), ..Default::default() });
        assert!(rgb_of(t.surface).0 < 0xef, "light: surface is a shade darker");
    }

    #[test]
    fn falls_back_to_alacritty_toml() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("alacritty.toml"), "[colors.primary]\nbackground = \"0x282828\"\nforeground = \"#ebdbb2\"\n[colors.normal]\nblue = \"#83a598\"\n").unwrap();
        let t = load_dir(d.path()).unwrap();
        assert_eq!(rgb_of(t.bg), (0x28, 0x28, 0x28));
        assert_eq!(rgb_of(t.accent), (0x83, 0xa5, 0x98));
    }

    #[test]
    fn signature_changes_with_the_theme() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("colors.toml"), "background = \"#000000\"\n").unwrap();
        let a = signature_of(d.path());
        fs::write(d.path().join("colors.toml"), "background = \"#ffffff\"\nmode = \"light\"\n").unwrap();
        assert_ne!(a, signature_of(d.path()));
    }

    #[test]
    fn hook_is_idempotent_and_reversible() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("hooks/theme-set");
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, "#!/bin/bash\nnotify-send hi\n").unwrap();
        assert!(hook_install(&p).unwrap());
        assert!(!hook_install(&p).unwrap());
        let s = fs::read_to_string(&p).unwrap();
        assert_eq!(s.matches("# parcel:").count(), 1);
        assert!(s.contains("notify-send hi"));
        assert!(hook_uninstall(&p).unwrap());
        assert_eq!(fs::read_to_string(&p).unwrap(), "#!/bin/bash\nnotify-send hi\n");
    }

    #[test]
    fn hook_install_creates_the_file() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("omarchy/hooks/theme-set");
        assert!(hook_install(&p).unwrap());
        assert!(fs::read_to_string(&p).unwrap().starts_with("#!/bin/sh\n"));
    }

    #[test]
    fn hook_install_rejects_unreadable_files() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("hooks/theme-set");
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        // Write non-UTF-8 bytes
        fs::write(&p, b"#!/bin/sh\n\xff\xfe\n").unwrap();
        let original = fs::read(&p).unwrap();
        // hook_install should return error, not overwrite
        assert!(hook_install(&p).is_err());
        // File must be unchanged
        assert_eq!(fs::read(&p).unwrap(), original);
    }
}
