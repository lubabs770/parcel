//! Run a backend command (argv, never a shell) and log failures.

use std::{fs::{self, OpenOptions}, io::Write, path::PathBuf, process::{Command, Stdio}, time::{SystemTime, UNIX_EPOCH}};

/// Ok(stdout) on success. On failure: Err(first non-empty stderr line), with
/// the full stderr appended to parcel.log.
pub fn run(argv: &[String], stdin: Option<&[u8]>) -> Result<String, String> {
    let (bin, args) = argv.split_first().ok_or("empty command")?;
    let mut child = Command::new(bin)
        .args(args)
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{bin}: {e}"))?;
    if let (Some(input), Some(mut pipe)) = (stdin, child.stdin.take()) {
        pipe.write_all(input).map_err(|e| format!("{bin}: {e}"))?;
    } // pipe dropped here → EOF for the child
    let out = child.wait_with_output().map_err(|e| format!("{bin}: {e}"))?;
    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    log(argv, &err);
    Err(err
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(String::from)
        .unwrap_or_else(|| format!("{bin} exited with {}", out.status)))
}

pub fn log_path() -> PathBuf {
    crate::config::state_dir().join("parcel.log")
}

fn log(argv: &[String], stderr: &str) {
    let path = log_path();
    let _ = fs::create_dir_all(path.parent().unwrap());
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let _ = writeln!(f, "[{ts}] {}\n{}", argv.join(" "), stderr.trim_end());
    }
}
