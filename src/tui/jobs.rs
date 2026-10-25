//! Backend jobs: what runs on a worker thread, and what came back.

use crate::backend::{Backend, Draft, Envelope, Flag};
use std::{fs, path::PathBuf};

pub enum Op {
    Flag { flag: Flag, on: bool },
    Move { target: String },
    Delete,
}

pub enum Undo {
    Flag { id: String, flag: Flag, prev: bool },
    /// `revert` places `env` back using `App::order` (the backend's original listing
    /// order), not a captured index — a raw index goes stale the moment a second
    /// removal lands next to this one before either confirms.
    Removed { env: Envelope },
}

pub enum Job {
    Mailboxes,
    /// `generation` is `App::list_gen` at spawn time: `done()` drops a response whose
    /// generation no longer matches, so a reload can't be clobbered by an older in-flight page.
    List { mailbox: String, query: Option<String>, page: u32, generation: u64 },
    Read { mailbox: String, id: String },
    /// `generation` is `App::list_gen` at spawn: a failed removal is put back only into
    /// the same listing, never into a reloaded one that already has the row.
    Triage { mailbox: String, id: String, op: Op, undo: Undo, generation: u64 },
    Draft { kind: Draft, mailbox: String, id: String },
    Send { path: PathBuf },
}

pub enum Done {
    Mailboxes(Result<Vec<String>, String>),
    List { mailbox: String, query: Option<String>, page: u32, generation: u64, res: Result<Vec<Envelope>, String> },
    Read { mailbox: String, id: String, res: Result<String, String> },
    Triage { mailbox: String, undo: Undo, generation: u64, res: Result<(), String> },
    Draft(Result<String, String>),
    Sent { path: PathBuf, res: Result<(), String> },
}

impl Job {
    pub(crate) fn run(self, be: &Backend) -> Done {
        match self {
            Job::Mailboxes => Done::Mailboxes(be.mailboxes()),
            Job::List { mailbox, query, page, generation } => {
                let res = match &query { Some(q) => be.search(&mailbox, q, page), None => be.list(&mailbox, page) };
                Done::List { mailbox, query, page, generation, res }
            }
            Job::Read { mailbox, id } => {
                let res = be.read(&mailbox, &id);
                Done::Read { mailbox, id, res }
            }
            Job::Triage { mailbox, id, op, undo, generation } => {
                let res = match op {
                    Op::Flag { flag, on } => be.set_flag(&mailbox, &id, flag, on),
                    Op::Move { target } => be.move_to(&mailbox, &id, &target),
                    Op::Delete => be.delete(&mailbox, &id),
                };
                Done::Triage { mailbox, undo, generation, res }
            }
            Job::Draft { kind, mailbox, id } => Done::Draft(be.draft(kind, &mailbox, &id)),
            Job::Send { path } => {
                let res = fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|raw| be.send(&raw));
                Done::Sent { path, res }
            }
        }
    }
}
