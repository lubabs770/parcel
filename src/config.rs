//! Config is a directory of small optional files layered over compiled-in
//! defaults: config.toml, keys.toml, backends/<name>.toml, themes/<name>.toml.

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{env, fs, io, path::{Path, PathBuf}};
use toml::{Table, Value};

pub const DEFAULT_CONFIG: &str = include_str!("../presets/config.toml");
pub const HIMALAYA: &str = include_str!("../presets/himalaya.toml");
pub const DEFAULT_KEYS: &str = include_str!("../presets/keys.toml");

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Settings {
    pub backend: String,
    pub theme: String,
    pub page_size: u32,
    pub inbox: String,
    pub archive: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Fields {
    pub items: String,
    pub id: String,
    pub subject: String,
    pub from: String,
    pub date: String,
    pub flags: String,
    pub flag_name: String,
    pub mailbox_items: String,
    pub mailbox_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct BackendSpec {
    pub mailboxes: Option<String>,
    pub list: Option<String>,
    pub search: Option<String>,
    pub read: Option<String>,
    pub flag: Option<String>,
    pub unflag: Option<String>,
    #[serde(rename = "move")]
    pub move_: Option<String>,
    pub delete: Option<String>,
    pub send: Option<String>,
    pub compose: Option<String>,
    pub reply: Option<String>,
    pub reply_all: Option<String>,
    pub forward: Option<String>,
    pub seen_flag: String,
    pub flagged_flag: String,
    pub fields: Fields,
}

// Hand-written so a custom backend that never mentions flags still reads them
// ("seen"/"flagged" as plain strings or `{name = ...}` objects).
impl Default for Fields {
    fn default() -> Self {
        let e = String::new;
        Self {
            items: e(), id: e(), subject: e(), from: e(), date: e(), flags: e(),
            flag_name: "name".into(), mailbox_items: e(), mailbox_name: e(),
        }
    }
}

impl Default for BackendSpec {
    fn default() -> Self {
        Self {
            mailboxes: None, list: None, search: None, read: None, flag: None, unflag: None, move_: None,
            delete: None, send: None, compose: None, reply: None, reply_all: None, forward: None,
            seen_flag: "seen".into(),
            flagged_flag: "flagged".into(),
            fields: Fields::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub dir: PathBuf,
    pub settings: Settings,
    pub backend: BackendSpec,
    pub keys: Table,
}

impl Config {
    pub fn load() -> Result<Self, String> {
        Self::load_from(&config_dir())
    }

    pub fn load_from(dir: &Path) -> Result<Self, String> {
        let settings: Settings = layered(DEFAULT_CONFIG, &dir.join("config.toml"))?;
        let path = dir.join("backends").join(format!("{}.toml", settings.backend));
        let preset = preset(&settings.backend);
        if preset.is_none() && !path.exists() {
            return Err(format!("backend {:?}: no built-in preset and no {}", settings.backend, path.display()));
        }
        let backend = layered(preset.unwrap_or(""), &path)?;
        let keys = read_table(&dir.join("keys.toml"))?;
        Ok(Self { dir: dir.to_path_buf(), settings, backend, keys })
    }
}

pub fn preset(name: &str) -> Option<&'static str> {
    (name == "himalaya").then_some(HIMALAYA)
}

/// Parse `base`, overlay the file at `path` (if any) key by key, deserialize.
fn layered<T: DeserializeOwned>(base: &str, path: &Path) -> Result<T, String> {
    let mut table: Table = toml::from_str(base).map_err(|e| format!("built-in default: {e}"))?;
    merge(&mut table, read_table(path)?);
    // If toml 1.x lacks Value::try_into, use `T::deserialize(Value::Table(table))`.
    Value::Table(table).try_into().map_err(|e| format!("{}: {e}", path.display()))
}

/// A TOML file as a table; a missing file is an empty table.
pub fn read_table(path: &Path) -> Result<Table, String> {
    match fs::read_to_string(path) {
        Ok(s) => toml::from_str(&s).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Table::new()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Deep-merge `over` into `base`: nested tables merge, anything else replaces.
pub fn merge(base: &mut Table, over: Table) {
    for (k, v) in over {
        match (base.get_mut(&k), v) {
            (Some(Value::Table(b)), Value::Table(o)) => merge(b, o),
            (_, v) => {
                base.insert(k, v);
            }
        }
    }
}

/// Write the defaults out for editing. Never overwrites an existing file. The himalaya
/// file is fully commented out, so it never shadows later fixes to the built-in preset.
pub fn init(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let himalaya = format!(
        "# Uncomment lines to override the built-in preset.\n{}",
        HIMALAYA.lines().map(|l| format!("# {l}\n")).collect::<String>()
    );
    let files = [("config.toml", DEFAULT_CONFIG), ("keys.toml", DEFAULT_KEYS), ("backends/himalaya.toml", himalaya.as_str())];
    let mut written = Vec::new();
    for (rel, body) in files {
        let path = dir.join(rel);
        if path.exists() {
            continue;
        }
        fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::write(&path, body).map_err(|e| format!("{}: {e}", path.display()))?;
        written.push(path);
    }
    Ok(written)
}

pub fn config_dir() -> PathBuf {
    env_dir("PARCEL_CONFIG_DIR").unwrap_or_else(|| xdg("XDG_CONFIG_HOME", ".config").join("parcel"))
}

pub fn state_dir() -> PathBuf {
    env_dir("PARCEL_STATE_DIR").unwrap_or_else(|| xdg("XDG_STATE_HOME", ".local/state").join("parcel"))
}

/// `$var`, or `~/<fallback>`. XDG on macOS too, like most terminal tools.
pub fn xdg(var: &str, fallback: &str) -> PathBuf {
    env_dir(var).unwrap_or_else(|| home().join(fallback))
}

pub fn home() -> PathBuf {
    env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

fn env_dir(var: &str) -> Option<PathBuf> {
    env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_dir_gives_himalaya_defaults() {
        let d = tempfile::tempdir().unwrap();
        let c = Config::load_from(d.path()).unwrap();
        assert_eq!(c.settings.backend, "himalaya");
        assert_eq!(c.settings.page_size, 50);
        assert!(c.backend.list.as_deref().unwrap().contains("envelope list"));
        assert_eq!(c.backend.fields.items, "envelopes");
        assert!(c.keys.is_empty());
    }

    #[test]
    fn a_file_overrides_key_by_key() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir(d.path().join("backends")).unwrap();
        fs::write(d.path().join("backends/himalaya.toml"), "read = \"x {id}\"\n[fields]\nid = \"uid\"\n").unwrap();
        let c = Config::load_from(d.path()).unwrap();
        assert_eq!(c.backend.read.as_deref(), Some("x {id}"));
        assert!(c.backend.list.is_some(), "untouched keys keep the preset");
        assert_eq!(c.backend.fields.id, "uid");
        assert_eq!(c.backend.fields.items, "envelopes");
    }

    #[test]
    fn custom_backend_needs_its_file() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("config.toml"), "backend = \"mycli\"\n").unwrap();
        assert!(Config::load_from(d.path()).unwrap_err().contains("no built-in preset"));
        fs::create_dir(d.path().join("backends")).unwrap();
        fs::write(d.path().join("backends/mycli.toml"), "list = \"mycli ls {mailbox}\"\n").unwrap();
        let c = Config::load_from(d.path()).unwrap();
        assert_eq!(c.backend.list.as_deref(), Some("mycli ls {mailbox}"));
        assert!(c.backend.read.is_none());
    }

    #[test]
    fn custom_backend_gets_flag_defaults() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("config.toml"), "backend = \"mycli\"\n").unwrap();
        fs::create_dir(d.path().join("backends")).unwrap();
        fs::write(d.path().join("backends/mycli.toml"), "list = \"mycli ls\"\n[fields]\nid = \"uid\"\n").unwrap();
        let b = Config::load_from(d.path()).unwrap().backend;
        assert_eq!((b.seen_flag.as_str(), b.flagged_flag.as_str(), b.fields.flag_name.as_str()), ("seen", "flagged", "name"));
        let h = Config::load_from(tempfile::tempdir().unwrap().path()).unwrap().backend;
        assert_eq!(h.fields.flag_name, "iana|raw", "the himalaya preset still overrides it");
    }

    #[test]
    fn bad_toml_names_the_file() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("config.toml"), "page_size = \"lots\"").unwrap();
        assert!(Config::load_from(d.path()).unwrap_err().contains("config.toml"));
    }

    #[test]
    fn init_writes_once() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(init(d.path()).unwrap().len(), 3);
        assert!(init(d.path()).unwrap().is_empty());
        let himalaya = d.path().join("backends/himalaya.toml");
        assert!(read_table(&himalaya).unwrap().is_empty(), "every line is commented out");
        assert!(fs::read_to_string(&himalaya).unwrap().contains("# search"), "but the preset is there to uncomment");
        assert!(Config::load_from(d.path()).unwrap().backend.search.is_some(), "the preset still applies");
    }
}
