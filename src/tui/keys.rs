//! Actions, their default keys, and keys.toml overrides.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;
use toml::Table;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Up, Down, Top, Bottom, Open, Back, NextPane, Search, Reply, ReplyAll, Forward, Compose,
    Star, ToggleRead, Delete, Move, Archive, Palette, Themes, Help, Quit,
}

/// (keys.toml name, default key, action, label)
pub const DEFAULTS: [(&str, &str, Action, &str); 21] = [
    ("up", "k", Action::Up, "move up"),
    ("down", "j", Action::Down, "move down"),
    ("top", "g", Action::Top, "top"),
    ("bottom", "G", Action::Bottom, "bottom"),
    ("open", "enter", Action::Open, "open"),
    ("back", "esc", Action::Back, "back"),
    ("next_pane", "tab", Action::NextPane, "next pane"),
    ("search", "/", Action::Search, "search"),
    ("reply", "r", Action::Reply, "reply"),
    ("reply_all", "R", Action::ReplyAll, "reply all"),
    ("forward", "f", Action::Forward, "forward"),
    ("compose", "c", Action::Compose, "compose"),
    ("star", "s", Action::Star, "star"),
    ("toggle_read", "u", Action::ToggleRead, "toggle read"),
    ("delete", "d", Action::Delete, "delete"),
    ("move", "m", Action::Move, "move to…"),
    ("archive", "a", Action::Archive, "archive"),
    ("palette", "ctrl+p", Action::Palette, "command palette"),
    ("themes", "t", Action::Themes, "themes"),
    ("help", "?", Action::Help, "help"),
    ("quit", "q", Action::Quit, "quit"),
];

pub struct Keymap {
    map: HashMap<(KeyCode, KeyModifiers), Action>,
    labels: HashMap<Action, String>,
}

impl Keymap {
    /// Defaults, then each `name = "key"` from keys.toml replaces that action's
    /// key (and wins any clash). Arrows, Home and End always move.
    pub fn new(overrides: &Table) -> Result<Self, String> {
        if let Some(bad) = overrides.keys().find(|k| !DEFAULTS.iter().any(|d| d.0 == k.as_str())) {
            return Err(format!("keys.toml: unknown action `{bad}`"));
        }
        let (mut map, mut labels) = (HashMap::new(), HashMap::new());
        for overridden in [false, true] {
            for (name, default, action, _) in DEFAULTS {
                let spec = match overrides.get(name) {
                    Some(toml::Value::String(s)) if overridden => s.as_str(),
                    Some(toml::Value::String(_)) => continue,
                    Some(_) => return Err(format!("keys.toml: `{name}` must be a string")),
                    None if overridden => continue,
                    None => default,
                };
                let key = parse(spec).ok_or_else(|| format!("keys.toml: can't parse key {spec:?} for `{name}`"))?;
                map.insert(key, action);
                labels.insert(action, spec.to_string());
            }
        }
        for (code, a) in [(KeyCode::Up, Action::Up), (KeyCode::Down, Action::Down), (KeyCode::Home, Action::Top), (KeyCode::End, Action::Bottom)] {
            map.entry((code, KeyModifiers::NONE)).or_insert(a);
        }
        Ok(Self { map, labels })
    }

    pub fn action(&self, k: KeyEvent) -> Option<Action> {
        // An uppercase char already implies shift; drop it so "G" matches.
        let mods = if matches!(k.code, KeyCode::Char(_)) { k.modifiers.difference(KeyModifiers::SHIFT) } else { k.modifiers };
        self.map.get(&(k.code, mods)).copied()
    }

    pub fn key_for(&self, a: Action) -> String {
        self.labels.get(&a).cloned().unwrap_or_default()
    }
}

pub fn parse(s: &str) -> Option<(KeyCode, KeyModifiers)> {
    let (mods, key) = if let Some(k) = s.strip_prefix("ctrl+") {
        (KeyModifiers::CONTROL, k)
    } else if let Some(k) = s.strip_prefix("alt+") {
        (KeyModifiers::ALT, k)
    } else {
        (KeyModifiers::NONE, s)
    };
    let code = match key {
        "enter" => KeyCode::Enter,
        "esc" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "backtab" => KeyCode::BackTab,
        "space" => KeyCode::Char(' '),
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pgup" => KeyCode::PageUp,
        "pgdown" => KeyCode::PageDown,
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        k if k.chars().count() == 1 => KeyCode::Char(k.chars().next()?),
        _ => return None,
    };
    Some((code, mods))
}

#[cfg(test)]
mod tests {
    use super::*;
    use KeyCode::Char;

    fn ev(c: KeyCode, m: KeyModifiers) -> KeyEvent {
        KeyEvent::new(c, m)
    }

    #[test]
    fn defaults() {
        let k = Keymap::new(&Table::new()).unwrap();
        assert_eq!(k.action(ev(Char('j'), KeyModifiers::NONE)), Some(Action::Down));
        assert_eq!(k.action(ev(Char('G'), KeyModifiers::SHIFT)), Some(Action::Bottom));
        assert_eq!(k.action(ev(Char('p'), KeyModifiers::CONTROL)), Some(Action::Palette));
        assert_eq!(k.action(ev(KeyCode::Down, KeyModifiers::NONE)), Some(Action::Down));
        assert_eq!(k.key_for(Action::Palette), "ctrl+p");
    }

    #[test]
    fn overrides_replace_and_win() {
        let t: Table = toml::from_str("star = \"S\"\ntoggle_read = \"d\"").unwrap();
        let k = Keymap::new(&t).unwrap();
        assert_eq!(k.action(ev(Char('s'), KeyModifiers::NONE)), None);
        assert_eq!(k.action(ev(Char('S'), KeyModifiers::SHIFT)), Some(Action::Star));
        assert_eq!(k.action(ev(Char('d'), KeyModifiers::NONE)), Some(Action::ToggleRead));
    }

    #[test]
    fn bad_overrides_error() {
        let t: Table = toml::from_str("nope = \"x\"").unwrap();
        assert!(Keymap::new(&t).err().unwrap().contains("unknown action `nope`"));
        let t: Table = toml::from_str("star = \"ctrl+banana\"").unwrap();
        assert!(Keymap::new(&t).err().unwrap().contains("can't parse"));
    }
}
