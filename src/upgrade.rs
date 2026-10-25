//! `parcel upgrade`: re-run the one-line installer into wherever this binary lives.
//! The installer verifies checksums and test-runs the download before replacing anything.

use std::process::Command;

pub const INSTALL_URL: &str = "https://raw.githubusercontent.com/lubabs770/parcel/main/install.sh";
const LATEST_API: &str = "https://api.github.com/repos/lubabs770/parcel/releases/latest";

pub fn latest_tag() -> Result<String, String> {
    let out = Command::new("curl")
        .args(["-fsSL", "-H", "Accept: application/vnd.github+json", LATEST_API])
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        return Err(format!("couldn't reach GitHub: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    v["tag_name"].as_str().map(String::from).ok_or_else(|| "no release published yet".into())
}

pub fn newer(current: &str, tag: &str) -> bool {
    let parse = |v: &str| {
        let mut it = v.trim_start_matches('v').split(['.', '-']).map(|p| p.parse::<u64>().unwrap_or(0));
        (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
    };
    parse(tag) > parse(current)
}

pub fn run(check: bool) -> Result<(), String> {
    let current = env!("CARGO_PKG_VERSION");
    let tag = latest_tag()?;
    if !newer(current, &tag) {
        println!("parcel v{current} is up to date");
        return Ok(());
    }
    if check {
        println!("{tag} is available (you have v{current}). Run `parcel upgrade`.");
        return Ok(());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or("can't find this binary's directory")?;
    // The URL is a constant passed as $1, never interpolated into the script.
    let st = Command::new("sh")
        .arg("-c")
        .arg("curl -fsSL \"$1\" | sh")
        .arg("sh")
        .arg(INSTALL_URL)
        .env("BIN_DIR", dir)
        .env("VERSION", &tag)
        .status()
        .map_err(|e| e.to_string())?;
    if st.success() { Ok(()) } else { Err(format!("installer exited with {st}")) }
}

#[cfg(test)]
mod tests {
    use super::newer;

    #[test]
    fn compares_semver_tags() {
        assert!(newer("0.1.0", "v0.2.0"));
        assert!(!newer("0.2.0", "v0.2.0"));
        assert!(!newer("0.10.0", "v0.9.1"));
        assert!(newer("0.9.1", "v0.10.0"));
    }
}
