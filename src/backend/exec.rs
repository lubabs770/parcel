//! Run a backend command (argv, never a shell) and log failures.

use std::{fs::{self, OpenOptions}, io::Write, path::PathBuf, process::{Command, Stdio}, time::{SystemTime, UNIX_EPOCH}};

/// Ok(stdout) on success. On failure: Err(first non-empty stderr line), with
/// the full stderr appended to parcel.log. Stdin is written on a separate thread
/// to prevent deadlock with large inputs.
pub fn run(argv: &[String], stdin: Option<&[u8]>) -> Result<String, String> {
    let (bin, args) = argv.split_first().ok_or("empty command")?;
    let mut child = Command::new(bin)
        .args(args)
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{bin}: {e}"))?;

    // Write stdin on a separate thread while main thread drains stdout/stderr.
    let stdin_result = if let (Some(input), Some(pipe)) = (stdin, child.stdin.take()) {
        std::thread::scope(|s| {
            let write_handle = s.spawn(move || {
                let mut p = pipe;
                p.write_all(input).map_err(|e| format!("{bin}: {e}"))
            });
            let out = child.wait_with_output().map_err(|e| format!("{bin}: {e}"))?;
            let stdin_err = write_handle.join().ok().and_then(|r| r.err());
            Ok::<_, String>((out, stdin_err))
        })
    } else {
        let out = child.wait_with_output().map_err(|e| format!("{bin}: {e}"))?;
        Ok((out, None))
    }?;

    let (out, stdin_err) = stdin_result;
    if out.status.success() {
        // Child succeeded. If stdin write failed, report that; otherwise return stdout.
        if let Some(err) = stdin_err {
            return Err(err);
        }
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }

    // Child failed: its stderr error wins, even if stdin write also failed.
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

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn large_stdin_doesnt_deadlock() {
        let input = vec![b'x'; 1 << 20]; // 1 MiB
        let result = run(&["cat".to_string()], Some(&input));
        assert!(result.is_ok(), "cat with 1 MiB input should succeed");
        let output = result.unwrap();
        assert_eq!(output.len(), 1 << 20, "output should be 1 MiB");
    }
}
