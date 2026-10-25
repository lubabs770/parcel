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
    Removed { at: usize, env: Envelope },
}

pub enum Job {
    Mailboxes,
    /// `generation` is `App::list_gen` at spawn time: `done()` drops a response whose
    /// generation no longer matches, so a reload can't be clobbered by an older in-flight page.
    List { mailbox: String, query: Option<String>, page: u32, generation: u64 },
    Read { mailbox: String, id: String },
    Triage { mailbox: String, id: String, op: Op, undo: Undo },
    Draft { kind: Draft, mailbox: String, id: String },
    Send { path: PathBuf },
}

pub enum Done {
    Mailboxes(Result<Vec<String>, String>),
    List { mailbox: String, query: Option<String>, page: u32, generation: u64, res: Result<Vec<Envelope>, String> },
    Read { id: String, res: Result<String, String> },
    Triage { mailbox: String, undo: Undo, res: Result<(), String> },
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
                Done::Read { id, res }
            }
            Job::Triage { mailbox, id, op, undo } => {
                let res = match op {
                    Op::Flag { flag, on } => be.set_flag(&mailbox, &id, flag, on),
                    Op::Move { target } => be.move_to(&mailbox, &id, &target),
                    Op::Delete => be.delete(&mailbox, &id),
                };
                Done::Triage { mailbox, undo, res }
            }
            Job::Draft { kind, mailbox, id } => Done::Draft(be.draft(kind, &mailbox, &id)),
            Job::Send { path } => {
                let res = fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|raw| be.send(&raw));
                Done::Sent { path, res }
            }
        }
    }
}
