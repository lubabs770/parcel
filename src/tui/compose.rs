//! Hand a draft to $EDITOR. Drafts live under the state dir until a send succeeds.

use std::{fs, path::{Path, PathBuf}, process::Command, time::{SystemTime, UNIX_EPOCH}};

/// $VISUAL / $EDITOR may carry arguments ("code -w"), so it runs through sh
/// with the draft path passed safely as "$1".
pub fn edit(path: &Path) -> Result<(), String> {
    let editor = std::env::var("VISUAL").or_else(|_| std::env::var("EDITOR")).unwrap_or_else(|_| "vi".into());
    let st = Command::new("sh")
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg("sh")
        .arg(path)
        .status()
        .map_err(|e| format!("{editor}: {e}"))?;
    if st.success() { Ok(()) } else { Err(format!("{editor} exited with {st}")) }
}

pub fn save(text: &str) -> Result<PathBuf, String> {
    let dir = crate::config::state_dir().join("drafts");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let path = dir.join(format!("{ts}.eml"));
    fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}
