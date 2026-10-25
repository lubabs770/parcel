//! All TUI state and every state transition. Rendering lives in view.rs.

use super::{compose, date, keys::{Action, Keymap, DEFAULTS}};
pub use super::jobs::{Done, Job, Op, Undo};
use crate::backend::{Backend, Draft, Envelope, Flag};
use crate::config::Config;
use crate::theme::{self, Theme};
use ratatui::crossterm::event::{Event, KeyEventKind};
use ratatui::{layout::Rect, widgets::ListState};
use std::{collections::VecDeque, fs, path::PathBuf, sync::{Arc, mpsc::Sender}, time::Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pane { Mailboxes, List, Reader }

pub enum Msg { Input(Event), Done(Done), Tick, Repaint }

pub enum Effect { Edit(PathBuf) }

pub enum Mode { Normal, Search(String), Picker(Picker), Confirm(PathBuf), Help }

pub struct Picker { pub kind: PickKind, pub query: String, pub items: Vec<String>, pub sel: usize }

pub enum PickKind { Move, Palette, Theme { before: String } }

pub struct App {
    pub cfg: Config,
    pub keys: Keymap,
    pub theme: Theme,
    pub theme_name: String,
    pub mailboxes: Vec<String>,
    pub mbox_sel: usize,
    pub mailbox: String,
    pub envelopes: Vec<Envelope>,
    /// Ids in the order the backend returned them for this listing (reset by `load_list`,
    /// extended by `prefetch`'s pages) — unlike `envelopes`, `remove()` never touches it.
    /// `revert` uses it to reinsert a failed triage at the right spot even when another
    /// removal next to it is still in flight, reverting out of order, or never comes back
    /// at all.
    pub(crate) order: Vec<String>,
    pub list_sel: usize,
    pub list_state: ListState,
    pub page: u32,
    pub more: bool,
    pub loading: bool,
    /// Bumped by every `load_list` (a reload under the same mailbox/query counts). `done`
    /// drops any `Done::List` whose job was spawned under an older generation.
    pub(crate) list_gen: u64,
    pub query: Option<String>,
    pub reading: Option<(String, String)>,
    pub scroll: u16,
    pub focus: Pane,
    pub mode: Mode,
    /// Set by `edited`: did the last $EDITOR round trip fail? Confirm's `s` refuses to send
    /// while this is true, since there's nothing new to send.
    pub(crate) edit_failed: bool,
    pub status: Option<(String, bool)>,
    pub busy: usize,
    pub tick: usize,
    pub now: i64,
    pub rects: Vec<(Pane, Rect)>,
    pub last_click: Option<(Instant, u16, u16)>,
    pub quit: bool,
    pub(crate) bodies: VecDeque<(String, String)>,
    pub(crate) backend: Arc<Backend>,
    pub(crate) tx: Sender<Msg>,
}

impl App {
    pub fn new(cfg: Config, tx: Sender<Msg>) -> Result<Self, String> {
        let keys = Keymap::new(&cfg.keys)?;
        let theme_name = cfg.settings.theme.clone();
        let theme = theme::load(&theme_name, &cfg.dir)?;
        let backend = Arc::new(Backend::new(cfg.backend.clone(), cfg.settings.page_size));
        Ok(Self {
            mailbox: cfg.settings.inbox.clone(),
            cfg, keys, theme, theme_name, backend, tx,
            mailboxes: vec![], mbox_sel: 0, envelopes: vec![], order: vec![], list_sel: 0, list_state: ListState::default(),
            page: 1, more: false, loading: false, list_gen: 0, query: None, reading: None, scroll: 0,
            focus: Pane::List, mode: Mode::Normal, edit_failed: false, status: None, busy: 0, tick: 0,
            now: date::now(), rects: vec![], last_click: None, quit: false, bodies: VecDeque::new(),
        })
    }

    pub fn start(&mut self) {
        self.spawn(Job::Mailboxes);
        self.load_list();
    }

    // ponytail: one short-lived thread per backend call; a pool only if someone fires hundreds at once.
    pub(crate) fn spawn(&mut self, job: Job) {
        self.busy += 1;
        let (tx, be) = (self.tx.clone(), self.backend.clone());
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Done(job.run(&be)));
        });
    }

    pub(crate) fn load_list(&mut self) {
        self.envelopes.clear();
        self.order.clear();
        self.list_gen += 1;
        (self.page, self.list_sel, self.more, self.loading) = (1, 0, false, true);
        self.spawn(Job::List { mailbox: self.mailbox.clone(), query: self.query.clone(), page: 1, generation: self.list_gen });
    }

    pub(crate) fn prefetch(&mut self) {
        if self.more && !self.loading && self.list_sel + 10 >= self.envelopes.len() {
            self.loading = true;
            self.spawn(Job::List {
                mailbox: self.mailbox.clone(),
                query: self.query.clone(),
                page: self.page + 1,
                generation: self.list_gen,
            });
        }
    }

    pub fn error(&mut self, e: String) {
        self.status = Some((e, true));
    }

    pub fn info(&mut self, s: impl Into<String>) {
        self.status = Some((s.into(), false));
    }

    pub fn update(&mut self, msg: Msg) -> Option<Effect> {
        match msg {
            Msg::Tick => {
                self.tick += 1;
                self.now = date::now();
                None
            }
            Msg::Repaint => None, // Task 12
            Msg::Done(d) => {
                self.busy = self.busy.saturating_sub(1);
                self.done(d)
            }
            Msg::Input(Event::Key(k)) if k.kind == KeyEventKind::Press => self.key(k),
            Msg::Input(_) => None, // Task 12 adds mouse
        }
    }

    fn done(&mut self, d: Done) -> Option<Effect> {
        match d {
            Done::Mailboxes(Ok(m)) => {
                if let Some(i) = m.iter().position(|n| n.eq_ignore_ascii_case(&self.mailbox)) {
                    self.mbox_sel = i;
                    self.mailbox = m[i].clone();
                }
                self.mailboxes = m;
            }
            Done::List { mailbox, query, page, generation, res } => {
                if !mailbox.eq_ignore_ascii_case(&self.mailbox) || query != self.query || generation != self.list_gen {
                    return None; // a stale page: the user moved on, or reloaded under this same key
                }
                if page != 1 && page != self.page + 1 {
                    return None; // an out-of-order page from this generation (a later one already landed)
                }
                self.loading = false;
                match res {
                    Ok(list) => {
                        self.more = list.len() as u32 >= self.cfg.settings.page_size;
                        self.page = page;
                        let ids = list.iter().map(|e| e.id.clone());
                        if page == 1 {
                            self.order = ids.collect();
                            self.envelopes = list;
                        } else {
                            self.order.extend(ids);
                            self.envelopes.extend(list);
                        }
                    }
                    Err(e) => self.error(e),
                }
            }
            Done::Read { id, res: Ok(body) } => {
                self.bodies.push_back((id.clone(), body.clone()));
                if self.bodies.len() > 50 {
                    self.bodies.pop_front();
                }
                if self.reading.as_ref().is_some_and(|(r, _)| *r == id) {
                    self.reading = Some((id, body));
                    self.scroll = 0;
                }
            }
            Done::Triage { res: Ok(()), .. } => {}
            Done::Triage { mailbox, undo, res: Err(e) } => {
                if mailbox.eq_ignore_ascii_case(&self.mailbox) {
                    self.revert(undo);
                }
                self.error(e);
            }
            Done::Draft(Ok(text)) => match compose::save(&text) {
                Ok(path) => return Some(Effect::Edit(path)),
                Err(e) => self.error(e),
            },
            Done::Sent { path, res: Ok(()) } => {
                let _ = fs::remove_file(path);
                self.info("sent ✓");
            }
            Done::Sent { path, res: Err(e) } => self.error(format!("{e} — draft kept at {}", path.display())),
            Done::Mailboxes(Err(e)) | Done::Read { res: Err(e), .. } | Done::Draft(Err(e)) => self.error(e),
        }
        None
    }

    /// The editor closed: ask what to do with the draft.
    pub fn edited(&mut self, path: PathBuf, res: Result<(), String>) {
        self.mode = Mode::Confirm(path);
        self.edit_failed = res.is_err();
        if let Err(e) = res {
            self.error(e);
        }
    }

    /// Is the backend able to do this? Actions without a template are refused (and shown dim).
    pub fn enabled(&self, a: Action) -> bool {
        let need = match a {
            Action::Search => "search",
            Action::Reply => "reply",
            Action::ReplyAll => "reply_all",
            Action::Forward => "forward",
            Action::Star | Action::ToggleRead => "flag",
            Action::Delete => "delete",
            Action::Move | Action::Archive => "move",
            _ => return true,
        };
        self.backend.has(need)
    }

    pub fn act(&mut self, a: Action) -> Option<Effect> {
        if !self.enabled(a) {
            self.error(format!("not available: the `{}` backend has no template for it", self.cfg.settings.backend));
            return None;
        }
        match a {
            Action::Quit => self.quit = true,
            Action::Up => self.step(-1),
            Action::Down => self.step(1),
            Action::Top => self.step(-(i32::MAX as isize)),
            Action::Bottom => self.step(i32::MAX as isize),
            Action::NextPane => {
                self.focus = match self.focus {
                    Pane::Mailboxes => Pane::List,
                    Pane::List if self.reading.is_some() => Pane::Reader,
                    Pane::List | Pane::Reader => Pane::Mailboxes,
                }
            }
            Action::Open => self.open(),
            Action::Back => match self.focus {
                Pane::Reader => self.focus = Pane::List,
                Pane::List if self.query.is_some() => {
                    self.query = None;
                    self.load_list();
                }
                Pane::List => self.focus = Pane::Mailboxes,
                Pane::Mailboxes => {}
            },
            Action::Search => self.mode = Mode::Search(self.query.clone().unwrap_or_default()),
            Action::Star => self.toggle(Flag::Flagged),
            Action::ToggleRead => self.toggle(Flag::Seen),
            Action::Delete => self.remove(Op::Delete),
            Action::Archive => {
                let target = self.cfg.settings.archive.clone();
                self.remove(Op::Move { target });
            }
            Action::Compose => self.draft(Draft::New),
            Action::Reply => self.draft(Draft::Reply),
            Action::ReplyAll => self.draft(Draft::ReplyAll),
            Action::Forward => self.draft(Draft::Forward),
            Action::Help => self.mode = Mode::Help,
            Action::Move => {
                let items = self.mailboxes.iter().filter(|m| **m != self.mailbox).cloned().collect();
                self.open_picker(PickKind::Move, items, 0);
            }
            Action::Themes => {
                let items = theme::names(&self.cfg.dir);
                let sel = items.iter().position(|n| *n == self.theme_name).unwrap_or(0);
                self.open_picker(PickKind::Theme { before: self.theme_name.clone() }, items, sel);
            }
            Action::Palette => {
                let mut items: Vec<String> = DEFAULTS
                    .iter()
                    .filter(|d| !matches!(d.2, Action::Up | Action::Down | Action::Top | Action::Bottom | Action::Open | Action::Back | Action::Palette))
                    .map(|d| d.3.to_string())
                    .collect();
                items.extend(self.mailboxes.iter().map(|m| format!("go to {m}")));
                self.open_picker(PickKind::Palette, items, 0);
            }
        }
        None
    }
}
