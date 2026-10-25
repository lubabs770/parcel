//! Floating pickers (move / palette / themes) and the help sheet.

use super::{app::{App, Mode, PickKind, Picker}, keys::DEFAULTS};
use ratatui::{prelude::*, widgets::{Block, Clear, List, ListItem, ListState, Padding}};

/// Case-insensitive subsequence match; lower = tighter. None = no match.
pub fn fuzzy(query: &str, cand: &str) -> Option<usize> {
    let c: Vec<char> = cand.to_lowercase().chars().collect();
    let (mut pos, mut score) = (0, 0);
    for q in query.to_lowercase().chars().filter(|c| !c.is_whitespace()) {
        let i = c.get(pos..)?.iter().position(|&x| x == q)?;
        score += i;
        pos += i + 1;
    }
    Some(score)
}

/// Indexes of matching items, best first (stable for ties).
pub fn filter(items: &[String], q: &str) -> Vec<usize> {
    let mut v: Vec<(usize, usize)> = items.iter().enumerate().filter_map(|(i, s)| fuzzy(q, s).map(|sc| (sc, i))).collect();
    v.sort();
    v.into_iter().map(|(_, i)| i).collect()
}

pub fn draw(f: &mut Frame, app: &App) {
    match &app.mode {
        Mode::Picker(p) => picker(f, app, p),
        Mode::Help => help(f, app),
        _ => {}
    }
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let (w, h) = (w.min(area.width), h.min(area.height));
    Rect { x: area.x + (area.width - w) / 2, y: area.y + (area.height - h) / 3, width: w, height: h }
}

fn sheet(app: &App, title: &str) -> Block<'static> {
    let t = app.theme;
    Block::new()
        .style(Style::new().bg(t.overlay).fg(t.fg))
        .padding(Padding::new(1, 1, 1, 0))
        .title(Line::from(format!(" {title} ")).style(Style::new().fg(t.bg).bg(t.accent).bold()))
}

fn picker(f: &mut Frame, app: &App, p: &Picker) {
    let t = app.theme;
    let shown = filter(&p.items, &p.query);
    let r = centered(f.area(), 52, (shown.len() as u16 + 5).clamp(7, 20));
    let title = match p.kind {
        PickKind::Move => "move to",
        PickKind::Palette => "commands",
        PickKind::Theme { .. } => "themes",
    };
    let block = sheet(app, title);
    let inner = block.inner(r);
    f.render_widget(Clear, r);
    f.render_widget(block, r);
    let [input, rest] = Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).areas(inner);
    f.render_widget(
        Line::from(vec![Span::styled("› ", Style::new().fg(t.accent)), Span::raw(p.query.clone()), Span::styled("▏", Style::new().fg(t.accent))]),
        input,
    );
    let items: Vec<ListItem> = shown
        .iter()
        .map(|&i| {
            let label = &p.items[i];
            let action = DEFAULTS.iter().find(|d| d.3 == label.as_str()).map(|d| d.2);
            let enabled = action.is_none_or(|a| app.enabled(a));
            let key = action.map(|a| app.keys.key_for(a)).unwrap_or_default();
            let style = if enabled { Style::new() } else { Style::new().fg(t.muted) };
            ListItem::new(Line::from(vec![Span::styled(label.clone(), style), Span::raw("  "), Span::styled(key, Style::new().fg(t.muted))]))
        })
        .collect();
    let mut st = ListState::default().with_selected(Some(p.sel));
    f.render_stateful_widget(List::new(items).highlight_style(Style::new().bg(t.selection).bold()), rest, &mut st);
}

fn help(f: &mut Frame, app: &App) {
    let t = app.theme;
    let rows = DEFAULTS.len().div_ceil(2);
    let r = centered(f.area(), 64, rows as u16 + 3);
    let block = sheet(app, "keys");
    let inner = block.inner(r);
    f.render_widget(Clear, r);
    f.render_widget(block, r);
    let cell = |i: usize| -> Vec<Span<'static>> {
        let Some((_, _, a, label)) = DEFAULTS.get(i) else { return vec![] };
        let dim = !app.enabled(*a);
        vec![
            Span::styled(format!("{:>7}  ", app.keys.key_for(*a)), Style::new().fg(if dim { t.muted } else { t.accent }).bold()),
            Span::styled(format!("{label:<20}"), Style::new().fg(if dim { t.muted } else { t.fg })),
        ]
    };
    let lines: Vec<Line> = (0..rows).map(|i| Line::from([cell(i), cell(i + rows)].concat())).collect();
    f.render_widget(Text::from(lines), inner);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_matches_subsequences_and_ranks_tight_ones_first() {
        assert!(fuzzy("arc", "Archive").is_some());
        assert!(fuzzy("xyz", "Archive").is_none());
        let items = vec!["go to Archive".to_string(), "archive".to_string()];
        assert_eq!(filter(&items, "arch"), [1, 0]);
        assert_eq!(filter(&items, ""), [0, 1]);
    }
}
