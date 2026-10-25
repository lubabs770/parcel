//! `parcel doctor`: is the backend reachable and the setup sane?

use crate::backend::Backend;
use crate::config::Config;
use std::{env, path::Path};

pub fn run() -> Result<(), String> {
    let mut ok = true;
    let mut check = |pass: bool, what: String| {
        println!("{} {what}", if pass { "✓" } else { "✗" });
        ok &= pass;
    };
    let cfg = match Config::load() {
        Ok(c) => {
            check(true, format!("config     {}", c.dir.display()));
            c
        }
        Err(e) => {
            check(false, format!("config     {e}"));
            return Err("doctor found problems".into());
        }
    };
    let be = Backend::new(cfg.backend.clone(), cfg.settings.page_size);
    for action in Backend::ACTIONS {
        match be.template(action) {
            Some(t) => {
                let bin = t.split_whitespace().next().unwrap_or("");
                check(on_path(bin), format!("{action:<10} {bin}"));
            }
            None => println!("· {action:<10} not configured (its key is disabled)"),
        }
    }
    match be.mailboxes() {
        Ok(m) => check(true, format!("mailboxes  {} found", m.len())),
        Err(e) => check(false, format!("mailboxes  {e}")),
    }
    if ok { Ok(()) } else { Err("doctor found problems".into()) }
}

/// Is `bin` runnable: an existing path, or a file in some $PATH dir?
pub fn on_path(bin: &str) -> bool {
    if bin.contains('/') {
        return Path::new(bin).is_file();
    }
    env::var_os("PATH").is_some_and(|p| env::split_paths(&p).any(|d| d.join(bin).is_file()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn finds_binaries_on_path() {
        assert!(super::on_path("sh"));
        assert!(super::on_path("/bin/sh"));
        assert!(!super::on_path("definitely-not-a-binary-xyz"));
    }
}
