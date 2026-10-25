//! Painted panes: layered surfaces instead of box lines.

use super::{app::{App, Mode, Pane}, date};
use crate::theme::Theme;
use ratatui::{prelude::*, widgets::{Block, List, ListItem, ListState, Padding, Paragraph, Wrap}};

const SPIN: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn draw(f: &mut Frame, app: &mut App) {
    let t = app.theme;
    let area = f.area();
    f.render_widget(Block::new().style(Style::new().bg(t.bg).fg(t.fg)), area);
    let [main, status] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);
    let panes = if main.width >= 100 {
        let [a, b, c] = Layout::horizontal([Constraint::Length(24), Constraint::Percentage(42), Constraint::Min(30)])
            .spacing(1)
            .areas(main);
        vec![(Pane::Mailboxes, a), (Pane::List, b), (Pane::Reader, c)]
    } else {
        vec![(app.focus, main)]
    };
    app.rects = panes.clone();
    for (pane, r) in panes {
        match pane {
            Pane::Mailboxes => mailboxes(f, app, r),
            Pane::List => list(f, app, r),
            Pane::Reader => reader(f, app, r),
        }
    }
    status_bar(f, app, status);
    super::overlay::draw(f, app);
}

fn panel(app: &App, pane: Pane, title: String) -> Block<'static> {
    let t = app.theme;
    let title_style = if app.focus == pane { Style::new().fg(t.bg).bg(t.accent).bold() } else { Style::new().fg(t.muted) };
    Block::new()
        .style(Style::new().bg(t.surface).fg(t.fg))
        .padding(Padding::horizontal(1))
        .title(Line::from(format!(" {title} ")).style(title_style))
}

fn mailboxes(f: &mut Frame, app: &App, r: Rect) {
    let t = app.theme;
    let items: Vec<ListItem> = app
        .mailboxes
        .iter()
        .map(|m| {
            let cur = *m == app.mailbox;
            ListItem::new(Line::from(vec![
                Span::styled(if cur { "▌" } else { " " }, Style::new().fg(t.accent)),
                Span::styled(m.clone(), if cur { Style::new().bold() } else { Style::new() }),
            ]))
        })
        .collect();
    let mut st = ListState::default().with_selected((app.focus == Pane::Mailboxes).then_some(app.mbox_sel));
    let list = List::new(items).block(panel(app, Pane::Mailboxes, "mailboxes".into())).highlight_style(Style::new().bg(t.selection));
    f.render_stateful_widget(list, r, &mut st);
}

fn list(f: &mut Frame, app: &mut App, r: Rect) {
    let t = app.theme;
    let title = match &app.query {
        Some(q) => format!("{} · “{q}”", app.mailbox),
        None => format!("{} · {}{}", app.mailbox, app.envelopes.len(), if app.more { "+" } else { "" }),
    };
    let block = panel(app, Pane::List, title);
    let inner = block.inner(r);
    let width = inner.width as usize;
    let items: Vec<ListItem> = app
        .envelopes
        .iter()
        .map(|e| {
            let mark = if e.flagged {
                Span::styled("⚑ ", Style::new().fg(t.flagged))
            } else if !e.seen {
                Span::styled("● ", Style::new().fg(t.unread))
            } else {
                Span::raw("  ")
            };
            let initial = e.from.chars().find(|c| c.is_alphanumeric()).map_or("?".into(), |c| c.to_uppercase().to_string());
            let when = date::relative(&e.date, app.now);
            let from_w = 16;
            let subj_w = width.saturating_sub(2 + 3 + 1 + from_w + 1 + 1 + when.chars().count());
            let name = if e.seen { Style::new().fg(t.fg) } else { Style::new().fg(t.fg).bold() };
            ListItem::new(Line::from(vec![
                mark,
                Span::styled(format!(" {initial} "), Style::new().fg(t.bg).bg(avatar(&t, &e.from))),
                Span::raw(" "),
                Span::styled(fit(&e.from, from_w), name),
                Span::raw(" "),
                Span::styled(fit(&e.subject, subj_w), Style::new().fg(if e.seen { t.muted } else { t.fg })),
                Span::raw(" "),
                Span::styled(when, Style::new().fg(t.muted)),
            ]))
        })
        .collect();
    let empty = items.is_empty();
    app.list_state.select((!empty).then_some(app.list_sel));
    let hl = if app.focus == Pane::List { t.selection } else { t.overlay };
    f.render_stateful_widget(List::new(items).block(block).highlight_style(Style::new().bg(hl)), r, &mut app.list_state);
    if empty {
        let msg = if app.loading { "loading…" } else { "nothing here ✨" };
        f.render_widget(Paragraph::new(msg).style(Style::new().fg(t.muted)).centered(), inner);
    }
}

fn reader(f: &mut Frame, app: &App, r: Rect) {
    let t = app.theme;
    let block = panel(app, Pane::Reader, "reading".into());
    let Some((id, body)) = &app.reading else {
        f.render_widget(Paragraph::new("\npick a message ✉").style(Style::new().fg(t.muted)).centered().block(block), r);
        return;
    };
    let mut lines = vec![];
    if let Some(e) = app.envelopes.iter().find(|e| e.id == *id) {
        lines.push(Line::from(vec![Span::styled("From  ", Style::new().fg(t.muted)), Span::styled(e.from.clone(), Style::new().bold())]));
        lines.push(Line::from(vec![Span::styled("Subj  ", Style::new().fg(t.muted)), Span::styled(e.subject.clone(), Style::new().fg(t.accent).bold())]));
        lines.push(Line::raw(""));
    }
    if body.is_empty() {
        lines.push(Line::styled("loading…", Style::new().fg(t.muted)));
    }
    lines.extend(body.lines().map(|l| {
        if l.starts_with('>') { Line::styled(l.to_string(), Style::new().fg(t.muted)) } else { Line::raw(l.to_string()) }
    }));
    f.render_widget(Paragraph::new(lines).block(block).wrap(Wrap { trim: false }).scroll((app.scroll, 0)), r);
}

/// The furthest the reader can scroll: its rendered rows (3 header rows, then each line
/// wrapped to the pane's inner width) minus the rows that fit. Without a reader rect yet,
/// the line count. ponytail: wraps by chars, not words or cell width, so a word-wrapped
/// line can run a row longer than counted.
pub(crate) fn max_scroll(app: &App) -> usize {
    let Some((id, body)) = &app.reading else { return 0 };
    let Some(&(_, r)) = app.rects.iter().find(|(p, _)| *p == Pane::Reader) else { return body.lines().count() };
    let inner = panel(app, Pane::Reader, String::new()).inner(r);
    let w = (inner.width as usize).max(1);
    let header = if app.envelopes.iter().any(|e| e.id == *id) { 3 } else { 0 };
    let rows: usize = body.lines().map(|l| l.chars().count().div_ceil(w).max(1)).sum();
    (header + rows.max(1)).saturating_sub(inner.height as usize)
}

fn status_bar(f: &mut Frame, app: &App, r: Rect) {
    let t = app.theme;
    f.render_widget(Block::new().style(Style::new().bg(t.surface)), r);
    let left: Vec<Span> = match &app.mode {
        Mode::Search(q) => vec![Span::styled(" / ", Style::new().fg(t.bg).bg(t.accent)), Span::raw(format!(" {q}▏"))],
        Mode::Confirm(_) => {
            let mut v = vec![
                Span::styled(" ✉ ", Style::new().fg(t.bg).bg(t.accent)),
                Span::styled(" s send · e edit · d discard", Style::new().fg(t.fg)),
            ];
            // Confirm never clears `status` on entry, so a kept-over editor-failure (or a
            // refused send) stays visible here — always as an error, in danger colour.
            if let Some((msg, _)) = &app.status {
                v.push(Span::raw("  "));
                v.push(Span::styled(msg.clone(), Style::new().fg(t.danger)));
            }
            v
        }
        _ => {
            let mut v = vec![if app.busy > 0 { Span::styled(format!(" {} ", SPIN[app.tick % SPIN.len()]), Style::new().fg(t.accent)) } else { Span::raw("   ") }];
            if let Some((msg, err)) = &app.status {
                v.push(Span::styled(msg.clone(), Style::new().fg(if *err { t.danger } else { t.ok })));
            }
            v
        }
    };
    let right = format!("{} · {}   ? help · q quit ", app.cfg.settings.backend, app.theme_name);
    let [l, rr] = Layout::horizontal([Constraint::Min(0), Constraint::Length(right.chars().count() as u16)]).areas(r);
    f.render_widget(Line::from(left), l);
    f.render_widget(Line::styled(right, Style::new().fg(t.muted)), rr);
}

/// Pad or cut to exactly `w` chars. ponytail: counts chars, not cell width; CJK/emoji may run one cell long.
fn fit(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n <= w {
        format!("{s:<w$}")
    } else if w == 0 {
        String::new()
    } else {
        s.chars().take(w - 1).chain(['…']).collect()
    }
}

fn avatar(t: &Theme, from: &str) -> Color {
    let h = from.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    [t.accent, t.unread, t.flagged, t.ok, t.danger][h as usize % 5]
}

#[cfg(test)]
mod tests {
    use super::draw;
    use crate::backend::Envelope;
    use crate::tui::testutil::harness;
    use ratatui::{Terminal, backend::TestBackend};

    fn env(id: &str, subject: &str, from: &str, date: &str, seen: bool, flagged: bool) -> Envelope {
        Envelope { id: id.into(), subject: subject.into(), from: from.into(), date: date.into(), seen, flagged }
    }

    #[test]
    fn renders_wide_and_narrow() {
        for (w, h) in [(120u16, 30u16), (80, 24)] {
            let mut hn = harness();
            let app = &mut hn.app;
            app.now = crate::tui::date::parse("2026-10-25T18:00:00Z").unwrap();
            app.mailboxes = vec!["Inbox".into(), "Sent".into(), "Archive".into()];
            app.mailbox = "Inbox".into();
            app.envelopes = vec![
                env("3", "Ordered 1 item: Furniture", "Amazon.com", "2026-10-25T16:00:00Z", false, false),
                env("2", "Review your keyboard", "Keychron", "2026-10-21T09:00:00Z", true, false),
                env("1", "Weekly build notes", "Linear", "2026-10-03T12:00:00Z", true, true),
            ];
            app.reading = Some(("3".into(), "Hi there,\n\nYour order shipped.\n> quoted line".into()));
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| draw(f, app)).unwrap();
            let screen = term.backend().to_string();
            assert!(screen.contains("Amazon.com") && screen.contains("⚑") && screen.contains("2h"));
            insta::assert_snapshot!(format!("main_{w}x{h}"), screen);
        }
    }
}
