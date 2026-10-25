//! Live Omarchy repaint (SIGUSR1 + a ~1s poll while theme == "auto") and mouse input.
//! Split out of app.rs/actions.rs, which are already at the ~300-line budget.

use super::app::{App, Pane};
use super::date;
use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Position;
use std::time::Instant;

impl App {
    pub(crate) fn on_tick(&mut self) {
        self.tick += 1;
        self.now = date::now();
        // Without the Omarchy hook, notice theme switches within ~1 s.
        if self.tick.is_multiple_of(10) && self.theme_name == "auto" {
            let sig = crate::theme::omarchy::signature();
            if sig != self.omarchy_sig {
                self.omarchy_sig = sig;
                self.set_theme("auto");
            }
        }
    }

    pub(crate) fn repaint(&mut self) {
        if self.theme_name == "auto" {
            self.omarchy_sig = crate::theme::omarchy::signature();
            self.set_theme("auto");
        }
    }

    pub(crate) fn mouse(&mut self, m: MouseEvent) {
        let Some(&(pane, r)) = self.rects.iter().find(|(_, r)| r.contains(Position::new(m.column, m.row))) else { return };
        match m.kind {
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                self.focus = pane;
                self.step(if m.kind == MouseEventKind::ScrollDown { 3 } else { -3 });
            }
            MouseEventKind::Down(MouseButton::Left) => {
                self.focus = pane;
                let top = r.y + 1; // the title row
                if m.row < top {
                    return;
                }
                let row = (m.row - top) as usize;
                match pane {
                    // ponytail: mailbox lists are short, so no scroll offset is tracked there.
                    Pane::Mailboxes if row < self.mailboxes.len() => self.mbox_sel = row,
                    Pane::List if self.list_state.offset() + row < self.envelopes.len() => {
                        self.list_sel = self.list_state.offset() + row;
                        self.prefetch();
                    }
                    _ => return,
                }
                let now = Instant::now();
                let double = self.last_click.is_some_and(|(t, x, y)| now.duration_since(t).as_millis() < 400 && (x, y) == (m.column, m.row));
                self.last_click = if double { None } else { Some((now, m.column, m.row)) };
                if double {
                    self.open();
                }
            }
            _ => {}
        }
    }
}
