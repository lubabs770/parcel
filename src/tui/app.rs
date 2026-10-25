//! All TUI state and every state transition. Rendering lives in view.rs.

use super::{compose, date, keys::{Action, Keymap}};
pub use super::jobs::{Done, Job, Op, Undo};
use crate::backend::{Backend, Envelope, Flag};
use crate::config::Config;
use crate::theme::{self, Theme};
#[allow(unused_imports)] // KeyModifiers gets its first caller in Task 10
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
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
    pub list_sel: usize,
    pub list_state: ListState,
    pub page: u32,
    pub more: bool,
    pub loading: bool,
    pub query: Option<String>,
    pub reading: Option<(String, String)>,
    pub scroll: u16,
    pub focus: Pane,
    pub mode: Mode,
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
            mailboxes: vec![], mbox_sel: 0, envelopes: vec![], list_sel: 0, list_state: ListState::default(),
            page: 1, more: false, loading: false, query: None, reading: None, scroll: 0,
            focus: Pane::List, mode: Mode::Normal, status: None, busy: 0, tick: 0, now: date::now(),
            rects: vec![], last_click: None, quit: false, bodies: VecDeque::new(),
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
        (self.page, self.list_sel, self.more, self.loading) = (1, 0, false, true);
        self.spawn(Job::List { mailbox: self.mailbox.clone(), query: self.query.clone(), page: 1 });
    }

    pub(crate) fn prefetch(&mut self) {
        if self.more && !self.loading && self.list_sel + 10 >= self.envelopes.len() {
            self.loading = true;
            self.spawn(Job::List { mailbox: self.mailbox.clone(), query: self.query.clone(), page: self.page + 1 });
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
            Done::List { mailbox, query, page, res } => {
                if !mailbox.eq_ignore_ascii_case(&self.mailbox) || query != self.query {
                    return None; // a stale page: the user moved on
                }
                self.loading = false;
                match res {
                    Ok(list) => {
                        self.more = list.len() as u32 >= self.cfg.settings.page_size;
                        self.page = page;
                        if page == 1 { self.envelopes = list } else { self.envelopes.extend(list) }
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
                if mailbox == self.mailbox {
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

    pub(crate) fn revert(&mut self, undo: Undo) {
        match undo {
            Undo::Flag { id, flag, prev } => {
                if let Some(e) = self.envelopes.iter_mut().find(|e| e.id == id) {
                    match flag {
                        Flag::Seen => e.seen = prev,
                        Flag::Flagged => e.flagged = prev,
                    }
                }
            }
            Undo::Removed { at, env } => {
                let at = at.min(self.envelopes.len());
                self.envelopes.insert(at, env);
            }
        }
    }

    /// The editor closed: ask what to do with the draft.
    pub fn edited(&mut self, path: PathBuf, res: Result<(), String>) {
        self.mode = Mode::Confirm(path);
        if let Err(e) = res {
            self.error(e);
        }
    }

    fn key(&mut self, k: KeyEvent) -> Option<Effect> {
        if !matches!(self.mode, Mode::Confirm(_)) {
            self.status = None;
        }
        match &mut self.mode {
            Mode::Normal => {}
            Mode::Help => {
                self.mode = Mode::Normal;
                return None;
            }
            Mode::Search(q) => {
                match k.code {
                    KeyCode::Enter => {
                        let q = std::mem::take(q);
                        self.mode = Mode::Normal;
                        self.query = (!q.trim().is_empty()).then_some(q);
                        self.focus = Pane::List;
                        self.load_list();
                    }
                    KeyCode::Esc => self.mode = Mode::Normal,
                    KeyCode::Backspace => {
                        q.pop();
                    }
                    KeyCode::Char(c) => q.push(c),
                    _ => {}
                }
                return None;
            }
            Mode::Confirm(path) => {
                let path = path.clone();
                match k.code {
                    KeyCode::Char('s' | 'y') | KeyCode::Enter => {
                        self.mode = Mode::Normal;
                        if self.backend.has("send") {
                            self.info("sending…");
                            self.spawn(Job::Send { path });
                        } else {
                            self.error(format!("no `send` template configured — draft kept at {}", path.display()));
                        }
                    }
                    KeyCode::Char('e') => {
                        self.mode = Mode::Normal;
                        return Some(Effect::Edit(path));
                    }
                    KeyCode::Char('d') | KeyCode::Esc => {
                        self.mode = Mode::Normal;
                        let _ = fs::remove_file(&path);
                        self.info("draft discarded");
                    }
                    _ => {}
                }
                return None;
            }
            Mode::Picker(_) => return self.picker_key(k),
        }
        let action = self.keys.action(k)?;
        self.act(action)
    }

    /// Picker behaviour arrives in Task 11; until then any key closes it.
    fn picker_key(&mut self, _k: KeyEvent) -> Option<Effect> {
        self.mode = Mode::Normal;
        None
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
            Action::Help => self.mode = Mode::Help,
            _ => {} // Tasks 10 and 11 fill these in
        }
        None
    }

    pub(crate) fn step(&mut self, d: isize) {
        let clamp = |i: usize, len: usize| if len == 0 { 0 } else { (i as isize).saturating_add(d).clamp(0, len as isize - 1) as usize };
        match self.focus {
            Pane::Mailboxes => self.mbox_sel = clamp(self.mbox_sel, self.mailboxes.len()),
            Pane::List => {
                self.list_sel = clamp(self.list_sel, self.envelopes.len());
                self.prefetch();
            }
            Pane::Reader => {
                let lines = self.reading.as_ref().map_or(0, |(_, b)| b.lines().count());
                self.scroll = clamp(self.scroll as usize, lines) as u16;
            }
        }
    }

    pub(crate) fn open(&mut self) {
        match self.focus {
            Pane::Mailboxes => {
                if let Some(m) = self.mailboxes.get(self.mbox_sel).cloned() {
                    self.goto(m);
                }
            }
            Pane::List => {
                let Some(env) = self.envelopes.get_mut(self.list_sel) else { return };
                env.seen = true;
                let id = env.id.clone();
                self.focus = Pane::Reader;
                self.scroll = 0;
                match self.bodies.iter().find(|(i, _)| *i == id) {
                    Some((_, body)) => self.reading = Some((id, body.clone())),
                    None => {
                        self.reading = Some((id.clone(), String::new()));
                        self.spawn(Job::Read { mailbox: self.mailbox.clone(), id });
                    }
                }
            }
            Pane::Reader => {}
        }
    }

    pub(crate) fn goto(&mut self, mailbox: String) {
        self.mbox_sel = self.mailboxes.iter().position(|m| *m == mailbox).unwrap_or(self.mbox_sel);
        self.mailbox = mailbox;
        self.query = None;
        self.reading = None;
        self.focus = Pane::List;
        self.load_list();
    }

    /// The message an action applies to: the open one in the reader, else the list selection.
    #[allow(dead_code)] // used from Task 10
    pub(crate) fn current_index(&self) -> Option<usize> {
        match (&self.reading, self.focus) {
            (Some((id, _)), Pane::Reader) => self.envelopes.iter().position(|e| e.id == *id),
            _ => (self.list_sel < self.envelopes.len()).then_some(self.list_sel),
        }
    }
}
