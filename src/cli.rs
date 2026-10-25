//! Every TUI action is also a command, with --json for scripts.

use crate::backend::{Backend, Envelope, exec};
use crate::config::{self, Config};
use clap::{Parser, Subcommand};
use std::{io::Read, path::PathBuf};

#[derive(Parser)]
#[command(name = "parcel", version, about = "A light, fast, yummy terminal mail client for any email CLI")]
pub struct Cli {
    /// Print machine-readable JSON
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub cmd: Option<Cmd>,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// List mailboxes
    Mailboxes,
    /// List envelopes in a mailbox
    List {
        #[arg(short, long)]
        mailbox: Option<String>,
        #[arg(short, long, default_value_t = 1)]
        page: u32,
    },
    /// Search a mailbox
    Search {
        query: String,
        #[arg(short, long)]
        mailbox: Option<String>,
        #[arg(short, long, default_value_t = 1)]
        page: u32,
    },
    /// Print a message
    Read {
        id: String,
        #[arg(short, long)]
        mailbox: Option<String>,
    },
    /// Send a raw RFC 5322 message from a file, or from stdin
    Send { file: Option<PathBuf> },
    /// Check the backend, its templates and the theme setup
    Doctor,
    /// Show the end of the error log
    Log {
        #[arg(short = 'n', long, default_value_t = 40)]
        lines: usize,
    },
    /// Config files
    #[command(subcommand)]
    Config(ConfigCmd),
    /// Omarchy integration
    #[command(subcommand)]
    Omarchy(OmarchyCmd),
    /// Upgrade to the latest release (or just --check)
    Upgrade {
        #[arg(long)]
        check: bool,
    },
}

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Write the default config files (never overwrites)
    Init,
    /// Print the config directory
    Path,
}

#[derive(Subcommand)]
pub enum OmarchyCmd {
    /// Omarchy's theme-set hook: repaint running parcels instantly on theme change
    #[command(subcommand)]
    Hook(HookCmd),
}

#[derive(Subcommand)]
pub enum HookCmd {
    Install,
    Uninstall,
}

pub fn run(cli: Cli) -> Result<(), String> {
    let Some(cmd) = cli.cmd else { return crate::tui::run(Config::load()?) };
    match &cmd {
        Cmd::Config(ConfigCmd::Init) => {
            for p in config::init(&config::config_dir())? {
                println!("wrote {}", p.display());
            }
            return Ok(());
        }
        Cmd::Config(ConfigCmd::Path) => {
            println!("{}", config::config_dir().display());
            return Ok(());
        }
        Cmd::Log { lines } => {
            let s = std::fs::read_to_string(exec::log_path()).unwrap_or_default();
            let all: Vec<&str> = s.lines().collect();
            all[all.len().saturating_sub(*lines)..].iter().for_each(|l| println!("{l}"));
            return Ok(());
        }
        Cmd::Doctor => return crate::doctor::run(),
        Cmd::Upgrade { check } => return crate::upgrade::run(*check),
        Cmd::Omarchy(OmarchyCmd::Hook(h)) => {
            let p = crate::theme::omarchy::hook_path();
            let changed = match h {
                HookCmd::Install => crate::theme::omarchy::hook_install(&p)?,
                HookCmd::Uninstall => crate::theme::omarchy::hook_uninstall(&p)?,
            };
            println!("{} {}", if changed { "updated" } else { "unchanged:" }, p.display());
            return Ok(());
        }
        _ => {}
    }
    let cfg = Config::load()?;
    let be = Backend::new(cfg.backend.clone(), cfg.settings.page_size);
    let mbox = |m: Option<String>| m.unwrap_or_else(|| cfg.settings.inbox.clone());
    match cmd {
        Cmd::Mailboxes => {
            let m = be.mailboxes()?;
            if cli.json { print_json(&m) } else { m.iter().for_each(|n| println!("{n}")) }
        }
        Cmd::List { mailbox, page } => print_envelopes(&be.list(&mbox(mailbox), page)?, cli.json),
        Cmd::Search { query, mailbox, page } => print_envelopes(&be.search(&mbox(mailbox), &query, page)?, cli.json),
        Cmd::Read { id, mailbox } => {
            let body = be.read(&mbox(mailbox), &id)?;
            if cli.json { print_json(&serde_json::json!({ "id": id, "body": body })) } else { print!("{body}") }
        }
        Cmd::Send { file } => {
            let raw = match file {
                Some(p) => std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?,
                None => {
                    let mut s = String::new();
                    std::io::stdin().read_to_string(&mut s).map_err(|e| e.to_string())?;
                    s
                }
            };
            be.send(&raw)?;
            if !cli.json {
                eprintln!("sent");
            }
        }
        _ => unreachable!("handled above"),
    }
    Ok(())
}

fn print_envelopes(list: &[Envelope], json: bool) {
    if json {
        return print_json(list);
    }
    for e in list {
        let mark = match (e.seen, e.flagged) {
            (_, true) => '⚑',
            (false, _) => '●',
            _ => ' ',
        };
        println!("{:>7} {mark} {:<24.24}  {}", e.id, e.from, e.subject);
    }
}

fn print_json<T: serde::Serialize + ?Sized>(v: &T) {
    println!("{}", serde_json::to_string_pretty(v).expect("serializable"));
}
