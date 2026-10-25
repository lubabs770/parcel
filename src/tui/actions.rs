//! Key handling, navigation, and mailbox/reader transitions — split out of `app.rs` to
//! keep it under ~300 lines. Triage, compose and the picker/palette land here too, in
//! Tasks 10–12.

use super::app::{App, Effect, Mode, Pane, PickKind, Picker};
use super::jobs::{Job, Op, Undo};
use super::keys::DEFAULTS;
use super::overlay::filter;
use crate::backend::{Draft, Flag};
use crate::theme;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::fs;

impl App {
    pub(crate) fn key(&mut self, k: KeyEvent) -> Option<Effect> {
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
                        if self.edit_failed {
                            self.error("editor failed — press e to edit again or d to discard".into());
                        } else if self.backend.has("send") {
                            self.mode = Mode::Normal;
                            self.info("sending…");
                            self.spawn(Job::Send { path });
                        } else {
                            self.mode = Mode::Normal;
                            self.error(format!("no `send` template configured — draft kept at {}", path.display()));
                        }
                    }
                    KeyCode::Char('e') => {
                        self.mode = Mode::Normal;
                        return Some(Effect::Edit(path));
                    }
                    KeyCode::Char('d') => {
                        self.mode = Mode::Normal;
                        let _ = fs::remove_file(&path);
                        self.info("draft discarded");
                    }
                    KeyCode::Esc => {
                        // Esc backs out of the prompt without touching the draft — only `d` discards.
                        self.mode = Mode::Normal;
                        self.info(format!("draft kept at {}", path.display()));
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

    fn picker_key(&mut self, k: KeyEvent) -> Option<Effect> {
        let Mode::Picker(p) = &mut self.mode else { return None };
        let shown = filter(&p.items, &p.query);
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        match k.code {
            KeyCode::Esc => {
                let before = match &p.kind {
                    PickKind::Theme { before } => Some(before.clone()),
                    _ => None,
                };
                self.mode = Mode::Normal;
                if let Some(b) = before {
                    self.set_theme(&b);
                }
                return None;
            }
            KeyCode::Enter => {
                let choice = shown.get(p.sel).map(|&i| p.items[i].clone());
                let kind = std::mem::replace(&mut p.kind, PickKind::Move);
                self.mode = Mode::Normal;
                return choice.and_then(|c| self.picked(kind, c));
            }
            KeyCode::Up | KeyCode::BackTab => p.sel = p.sel.saturating_sub(1),
            KeyCode::Char('p' | 'k') if ctrl => p.sel = p.sel.saturating_sub(1),
            KeyCode::Down | KeyCode::Tab => p.sel = (p.sel + 1).min(shown.len().saturating_sub(1)),
            KeyCode::Char('n' | 'j') if ctrl => p.sel = (p.sel + 1).min(shown.len().saturating_sub(1)),
            KeyCode::Backspace => {
                p.query.pop();
                p.sel = 0;
            }
            KeyCode::Char(c) => {
                p.query.push(c);
                p.sel = 0;
            }
            _ => {}
        }
        // Live preview while browsing themes (name computed first: set_theme needs &mut self).
        let preview = match &self.mode {
            Mode::Picker(Picker { kind: PickKind::Theme { .. }, items, query, sel }) => {
                filter(items, query).get(*sel).map(|&i| items[i].clone())
            }
            _ => None,
        };
        if let Some(name) = preview {
            self.set_theme(&name);
        }
        None
    }

    fn picked(&mut self, kind: PickKind, choice: String) -> Option<Effect> {
        match kind {
            PickKind::Move => self.remove(Op::Move { target: choice }),
            PickKind::Theme { .. } => {
                self.set_theme(&choice);
                self.info(format!("theme {choice} — put theme = \"{choice}\" in config.toml to keep it"));
            }
            PickKind::Palette => {
                if let Some(m) = choice.strip_prefix("go to ") {
                    self.goto(m.to_string());
                } else if let Some(d) = DEFAULTS.iter().find(|d| d.3 == choice) {
                    return self.act(d.2);
                }
            }
        }
        None
    }

    pub fn set_theme(&mut self, name: &str) {
        match theme::load(name, &self.cfg.dir) {
            Ok(t) => {
                self.theme = t;
                self.theme_name = name.to_string();
            }
            Err(e) => self.error(e),
        }
    }

    pub(crate) fn open_picker(&mut self, kind: PickKind, items: Vec<String>, sel: usize) {
        self.mode = Mode::Picker(Picker { kind, query: String::new(), items, sel });
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

    fn goto(&mut self, mailbox: String) {
        self.mbox_sel = self.mailboxes.iter().position(|m| *m == mailbox).unwrap_or(self.mbox_sel);
        self.mailbox = mailbox;
        self.query = None;
        self.reading = None;
        self.focus = Pane::List;
        self.load_list();
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
            Undo::Removed { env } => {
                // Where `env.id` sat in the backend's listing order, counting only ids
                // still present now — so it lands in the right slot regardless of
                // whether some other in-flight removal next to it has reverted yet.
                let at = match self.order.iter().position(|id| *id == env.id) {
                    Some(p) => self.order[..p].iter().filter(|id| self.envelopes.iter().any(|e| e.id == **id)).count(),
                    None => self.envelopes.len(),
                };
                self.envelopes.insert(at, env);
            }
        }
    }

    /// The message an action applies to: the open one in the reader, else the list selection.
    pub(crate) fn current_index(&self) -> Option<usize> {
        match (&self.reading, self.focus) {
            (Some((id, _)), Pane::Reader) => self.envelopes.iter().position(|e| e.id == *id),
            _ => (self.list_sel < self.envelopes.len()).then_some(self.list_sel),
        }
    }

    pub(crate) fn toggle(&mut self, flag: Flag) {
        let Some(i) = self.current_index() else { return };
        let e = &mut self.envelopes[i];
        let slot = match flag {
            Flag::Seen => &mut e.seen,
            Flag::Flagged => &mut e.flagged,
        };
        let prev = *slot;
        *slot = !prev;
        let id = e.id.clone();
        let mailbox = self.mailbox.clone();
        self.spawn(Job::Triage { mailbox, id: id.clone(), op: Op::Flag { flag, on: !prev }, undo: Undo::Flag { id, flag, prev } });
    }

    pub(crate) fn remove(&mut self, op: Op) {
        let Some(i) = self.current_index() else { return };
        let env = self.envelopes.remove(i);
        if self.reading.as_ref().is_some_and(|(id, _)| *id == env.id) {
            self.reading = None;
            self.focus = Pane::List;
        }
        self.list_sel = self.list_sel.min(self.envelopes.len().saturating_sub(1));
        let mailbox = self.mailbox.clone();
        self.spawn(Job::Triage { mailbox, id: env.id.clone(), op, undo: Undo::Removed { env } });
        self.prefetch();
    }

    pub(crate) fn draft(&mut self, kind: Draft) {
        let id = match kind {
            Draft::New => String::new(),
            _ => match self.current_index() {
                Some(i) => self.envelopes[i].id.clone(),
                None => return,
            },
        };
        self.info("preparing draft…");
        self.spawn(Job::Draft { kind, mailbox: self.mailbox.clone(), id });
    }
}
