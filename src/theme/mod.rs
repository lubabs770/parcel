//! Colour tokens. Built-ins, your own themes/<name>.toml, or Omarchy's.

pub mod builtin;

use ratatui::style::Color;
use serde::Deserialize;
use std::{fs, path::Path};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub bg: Color,
    pub surface: Color,
    pub overlay: Color,
    pub fg: Color,
    pub muted: Color,
    pub accent: Color,
    pub unread: Color,
    pub flagged: Color,
    pub danger: Color,
    pub ok: Color,
    pub selection: Color,
    pub border: Color,
}

/// themes/<name>.toml: the 12 tokens as hex strings.
#[derive(Deserialize)]
struct ThemeFile {
    bg: String,
    surface: String,
    overlay: String,
    fg: String,
    muted: String,
    accent: String,
    unread: String,
    flagged: String,
    danger: String,
    ok: String,
    selection: String,
    border: String,
}

impl Theme {
    /// Tokens in field order: bg surface overlay fg muted accent unread flagged danger ok selection border.
    pub fn from_hex(h: [&str; 12]) -> Result<Self, String> {
        let c = |i: usize| rgb(h[i]).map(|(r, g, b)| Color::Rgb(r, g, b)).ok_or_else(|| format!("bad colour {:?}", h[i]));
        Ok(Self {
            bg: c(0)?, surface: c(1)?, overlay: c(2)?, fg: c(3)?, muted: c(4)?, accent: c(5)?,
            unread: c(6)?, flagged: c(7)?, danger: c(8)?, ok: c(9)?, selection: c(10)?, border: c(11)?,
        })
    }
}

/// "#rrggbb" or "0xrrggbb".
pub fn rgb(s: &str) -> Option<(u8, u8, u8)> {
    let h = s.trim().trim_start_matches('#').trim_start_matches("0x");
    if h.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(h, 16).ok()?;
    Some(((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

pub fn load(name: &str, config_dir: &Path) -> Result<Theme, String> {
    if name == "auto" {
        return Ok(builtin::default()); // Task 7 makes this follow Omarchy
    }
    if let Some(t) = builtin::get(name) {
        return Ok(t);
    }
    let path = config_dir.join("themes").join(format!("{name}.toml"));
    let s = fs::read_to_string(&path).map_err(|e| format!("theme {name:?}: {e}"))?;
    let f: ThemeFile = toml::from_str(&s).map_err(|e| format!("{}: {e}", path.display()))?;
    Theme::from_hex([
        &f.bg, &f.surface, &f.overlay, &f.fg, &f.muted, &f.accent, &f.unread, &f.flagged, &f.danger, &f.ok,
        &f.selection, &f.border,
    ])
    .map_err(|e| format!("{}: {e}", path.display()))
}

/// "auto", the built-ins, then your themes/*.toml.
pub fn names(config_dir: &Path) -> Vec<String> {
    let mut v: Vec<String> =
        std::iter::once("auto".to_string()).chain(builtin::THEMES.iter().map(|(n, _)| n.to_string())).collect();
    if let Ok(rd) = fs::read_dir(config_dir.join("themes")) {
        let mut user: Vec<String> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "toml"))
            .filter_map(|p| p.file_stem()?.to_str().map(String::from))
            .filter(|n| !v.contains(n))
            .collect();
        user.sort();
        v.extend(user);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn builtins_parse() {
        for (name, _) in builtin::THEMES {
            assert!(builtin::get(name).is_some(), "{name}");
        }
    }

    #[test]
    fn hex_forms() {
        assert_eq!(rgb("#1a1b26"), Some((0x1a, 0x1b, 0x26)));
        assert_eq!(rgb("0x282828"), Some((0x28, 0x28, 0x28)));
        assert_eq!(rgb("nope"), None);
    }

    #[test]
    fn user_theme_files() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir(d.path().join("themes")).unwrap();
        let toml = |bg: &str| {
            format!(
                "bg=\"{bg}\"\nsurface=\"#111111\"\noverlay=\"#222222\"\nfg=\"#eeeeee\"\nmuted=\"#888888\"\naccent=\"#ff00aa\"\n\
                 unread=\"#00ffff\"\nflagged=\"#ffff00\"\ndanger=\"#ff0000\"\nok=\"#00ff00\"\nselection=\"#333333\"\nborder=\"#444444\"\n"
            )
        };
        fs::write(d.path().join("themes/mine.toml"), toml("#000000")).unwrap();
        assert_eq!(load("mine", d.path()).unwrap().accent, Color::Rgb(0xff, 0x00, 0xaa));
        fs::write(d.path().join("themes/bad.toml"), toml("nope")).unwrap();
        assert!(load("bad", d.path()).unwrap_err().contains("bad colour"));
        assert!(load("missing", d.path()).is_err());
        let names = names(d.path());
        assert_eq!(names[0], "auto");
        assert!(names.contains(&"mine".to_string()) && names.contains(&"gruvbox".to_string()));
    }
}
