use super::{app::*, keys::Action, testutil::{harness, harness_with_failing_send}};
use crate::backend::Envelope;
use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::layout::Rect;

fn env(id: &str) -> Envelope {
    Envelope { id: id.into(), subject: "s".into(), from: "f".into(), date: "2020-01-01T00:00:00Z".into(), seen: false, flagged: false }
}

#[test]
fn start_loads_mailboxes_and_inbox() {
    let h = harness().started();
    assert_eq!(h.app.mailboxes, ["Inbox", "Archive"]);
    assert_eq!(h.app.mailbox, "Inbox", "\"inbox\" adopts the backend's spelling");
    assert_eq!(h.app.envelopes.len(), 2);
    assert_eq!(h.app.busy, 0);
    assert!(!h.app.more, "2 < page_size → no more pages");
}

#[test]
fn open_marks_seen_and_reads() {
    let mut h = harness().started();
    h.app.focus = Pane::List;
    h.app.list_sel = 1;
    h.app.act(Action::Open);
    assert!(h.app.envelopes[1].seen);
    assert_eq!(h.app.focus, Pane::Reader);
    h.settle();
    assert!(h.app.reading.as_ref().unwrap().1.contains("Hi there"));
}

#[test]
fn stale_list_results_dropped_for_wrong_mailbox() {
    let mut h = harness();
    h.app.mailbox = "Archive".into();
    h.app.loading = true;
    let generation = h.app.list_gen;
    h.app.update(Msg::Done(Done::List { mailbox: "Inbox".into(), query: None, page: 1, generation, res: Ok(vec![env("9")]) }));
    assert!(h.app.envelopes.is_empty(), "a page for a mailbox the user has left is dropped, not shown");
    assert!(h.app.loading, "loading isn't cleared by a result that was dropped");
}

#[test]
fn stale_list_results_dropped_for_an_old_generation() {
    let mut h = harness().started();
    let before = h.app.envelopes.len();
    let old_gen = h.app.list_gen;
    // Simulate a same-key reload (Enter on the mailbox, or `/q` → back → `/q`): it bumps
    // the generation, so the page-2 prefetch that was already in flight under the old
    // generation must be dropped when it lands, not appended onto the fresh list.
    h.app.list_gen += 1;
    let mailbox = h.app.mailbox.clone();
    h.app.update(Msg::Done(Done::List { mailbox, query: None, page: 2, generation: old_gen, res: Ok(vec![env("stale")]) }));
    assert_eq!(h.app.envelopes.len(), before, "an old-generation page is dropped, not extended onto the fresh list");
    assert_eq!(h.app.page, 1, "page isn't bumped by a dropped result");
}

#[test]
fn editor_round_trip_sends_and_cleans_up() {
    let mut h = harness().started();
    let path = crate::tui::compose::save("To: a@x.io\n\nhi\n").unwrap();
    h.app.edited(path.clone(), Ok(()));
    assert!(matches!(h.app.mode, Mode::Confirm(_)));
    h.press(KeyCode::Char('s'), KeyModifiers::NONE);
    h.settle();
    assert!(!path.exists(), "draft removed after a successful send");
    assert_eq!(h.app.status, Some(("sent ✓".into(), false)));
}

#[test]
fn confirm_esc_keeps_the_draft() {
    let mut h = harness();
    let path = crate::tui::compose::save("To: a@x.io\n\nhi\n").unwrap();
    h.app.edited(path.clone(), Ok(()));
    h.press(KeyCode::Esc, KeyModifiers::NONE);
    assert!(path.exists(), "Esc keeps the draft file — only `d` discards");
    assert!(matches!(h.app.mode, Mode::Normal));
    assert_eq!(h.app.status, Some((format!("draft kept at {}", path.display()), false)));
}

#[test]
fn confirm_send_refuses_after_a_failed_edit() {
    let mut h = harness();
    let path = crate::tui::compose::save("To: a@x.io\n\nhi\n").unwrap();
    h.app.edited(path.clone(), Err("editor exploded".into()));
    h.press(KeyCode::Char('s'), KeyModifiers::NONE);
    assert!(matches!(h.app.mode, Mode::Confirm(_)), "stays in Confirm so e/d still work");
    assert!(path.exists(), "nothing is sent while the last edit failed");
    assert_eq!(h.app.status.as_ref().map(|(_, is_err)| *is_err), Some(true));
}

#[test]
fn failed_send_keeps_the_draft_and_names_its_path() {
    let mut h = harness_with_failing_send();
    let path = crate::tui::compose::save("To: a@x.io\n\nhi\n").unwrap();
    h.app.edited(path.clone(), Ok(()));
    h.press(KeyCode::Char('s'), KeyModifiers::NONE);
    h.settle();
    assert!(path.exists(), "a failed send keeps the draft");
    let (msg, is_err) = h.app.status.clone().unwrap();
    assert!(is_err);
    assert!(msg.contains(&path.display().to_string()), "status names the draft path: {msg}");
}

#[test]
fn star_is_optimistic_and_sticks() {
    let mut h = harness().started();
    h.app.act(Action::Star);
    assert!(h.app.envelopes[0].flagged, "flips before the backend answers");
    h.settle();
    assert!(h.app.envelopes[0].flagged);
    assert!(std::fs::read_to_string(h.dir.path().join("argv.log")).unwrap().contains("flag 1 flagged"));
}

#[test]
fn failed_delete_puts_the_row_back() {
    let mut h = harness().started();
    h.app.act(Action::Delete);
    assert_eq!(h.app.envelopes.len(), 1);
    h.settle();
    assert_eq!(h.app.envelopes.len(), 2);
    assert_eq!(h.app.envelopes[0].id, "1");
    assert!(h.app.status.as_ref().is_some_and(|(m, err)| *err && m.contains("boom")));
}

#[test]
fn archive_moves_to_the_configured_mailbox() {
    let mut h = harness().started();
    h.app.act(Action::Archive);
    h.settle();
    assert_eq!(h.app.envelopes.len(), 1);
    assert!(std::fs::read_to_string(h.dir.path().join("argv.log")).unwrap().contains("move 1 Archive"));
}

#[test]
fn reply_fetches_a_draft_for_the_editor() {
    let mut h = harness().started();
    assert!(h.app.act(Action::Reply).is_none());
    let Some(Effect::Edit(path)) = h.settle() else { panic!("expected the editor") };
    assert!(std::fs::read_to_string(&path).unwrap().starts_with("Subject: Re:"));
}

#[test]
fn missing_template_refuses_the_key() {
    let mut h = harness().started();
    h.press(KeyCode::Char('R'), KeyModifiers::SHIFT); // fake backend has no reply_all
    assert_eq!(h.app.busy, 0);
    assert!(h.app.status.as_ref().is_some_and(|(m, err)| *err && m.contains("not available")));
}

#[test]
fn two_failed_deletes_restore_original_order() {
    let mut h = harness().started();
    h.app.act(Action::Delete); // removes row 0 (id "1")
    h.app.act(Action::Delete); // id "2" is now at row 0
    h.settle();
    h.settle();
    let ids: Vec<&str> = h.app.envelopes.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["1", "2"], "both deletes failed; the rows must come back in their original order");
}

#[test]
fn move_picker_moves_the_selection() {
    let mut h = harness().started();
    h.app.act(Action::Move);
    let Mode::Picker(p) = &h.app.mode else { panic!("picker") };
    assert_eq!(p.items, ["Archive"], "current mailbox is excluded");
    h.type_str("arc");
    h.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(h.app.envelopes.len(), 1);
    h.settle();
    assert!(std::fs::read_to_string(h.dir.path().join("argv.log")).unwrap().contains("move 1 Archive"));
}

#[test]
fn theme_picker_previews_live_and_esc_restores() {
    let mut h = harness().started();
    h.app.act(Action::Themes);
    h.type_str("gruv");
    assert_eq!(h.app.theme_name, "gruvbox");
    h.press(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(h.app.theme_name, "tokyo-night");
}

#[test]
fn palette_runs_actions_and_jumps_to_mailboxes() {
    let mut h = harness().started();
    h.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
    h.type_str("star");
    h.press(KeyCode::Enter, KeyModifiers::NONE);
    assert!(h.app.envelopes[0].flagged);
    h.settle();
    h.press(KeyCode::Char('p'), KeyModifiers::CONTROL);
    h.type_str("go to arch");
    h.press(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(h.app.mailbox, "Archive");
}

#[test]
fn search_runs_the_search_template() {
    let mut h = harness().started();
    h.press(KeyCode::Char('/'), KeyModifiers::NONE);
    h.type_str("ada");
    h.press(KeyCode::Enter, KeyModifiers::NONE);
    h.settle();
    assert_eq!(h.app.query.as_deref(), Some("ada"));
    assert!(std::fs::read_to_string(h.dir.path().join("argv.log")).unwrap().contains("list Inbox ada"));
}

#[test]
fn click_selects_and_double_click_opens() {
    use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let mut h = harness().started();
    h.app.rects = vec![(Pane::List, Rect::new(0, 0, 60, 10))];
    let click = MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: 5, row: 2, modifiers: KeyModifiers::NONE };
    h.app.update(Msg::Input(Event::Mouse(click)));
    assert_eq!((h.app.focus, h.app.list_sel), (Pane::List, 1));
    h.app.update(Msg::Input(Event::Mouse(click)));
    assert_eq!(h.app.reading.as_ref().map(|r| r.0.as_str()), Some("2"));
}

#[test]
fn repaint_reloads_auto_theme_only() {
    let mut h = harness();
    h.app.update(Msg::Repaint);
    assert_eq!(h.app.theme_name, "tokyo-night", "a fixed theme ignores Omarchy");
    h.app.set_theme("auto");
    h.app.update(Msg::Repaint);
    assert_eq!(h.app.theme_name, "auto");
    assert!(h.app.status.is_none());
}

#[test]
fn cached_body_is_per_mailbox() {
    let mut h = harness().started();
    h.app.act(Action::Open); // Inbox id "1" → cached
    h.settle();
    h.app.mailbox = "Archive".into();
    h.app.load_list();
    h.settle();
    h.app.focus = Pane::List;
    h.app.act(Action::Open); // Archive id "1": a different message with the same id
    assert_eq!(h.app.reading, Some(("1".into(), String::new())), "Inbox's body is not served for Archive's id 1");
    assert_eq!(h.app.busy, 1, "Archive's body is fetched");
    h.app.update(Msg::Done(Done::Read { mailbox: "Inbox".into(), id: "1".into(), res: Ok("inbox body".into()) }));
    assert_eq!(h.app.reading, Some(("1".into(), String::new())), "a late Inbox read doesn't fill Archive's reader");
    h.settle();
    assert!(h.app.reading.as_ref().unwrap().1.contains("Hi there"), "Archive's own read does");
}

#[test]
fn reader_scrolls_to_the_last_wrapped_row() {
    let mut h = harness().started();
    // inner 10 wide x 4 high; 3 header rows + 3 + 1 + 1 + 1 body rows = 9 → max scroll 5
    h.app.reading = Some(("1".into(), "0123456789abcdefghijk\n\nshort\nx".into()));
    h.app.focus = Pane::Reader;
    h.app.rects = vec![(Pane::Reader, Rect::new(0, 0, 12, 5))];
    h.app.act(Action::Bottom);
    assert_eq!(h.app.scroll, 5);
    h.app.rects.clear();
    h.app.act(Action::Bottom);
    assert_eq!(h.app.scroll, 4, "no rect → falls back to the line count");
}

#[test]
fn stale_failed_delete_does_not_duplicate_after_reload() {
    let mut h = harness().started();
    h.app.act(Action::Delete); // the fake's delete fails
    h.app.load_list(); // reload before the delete settles
    let mut msgs = vec![h.rx.recv().unwrap(), h.rx.recv().unwrap()];
    // Worst order: the fresh list arrives first, then the stale failure.
    msgs.sort_by_key(|m| matches!(m, Msg::Done(Done::Triage { .. })));
    for m in msgs {
        h.app.update(m);
    }
    let ids: Vec<&str> = h.app.envelopes.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["1", "2"], "no duplicate row");
    assert!(h.app.status.as_ref().is_some_and(|(m, err)| *err && m.contains("boom")), "the error still shows");
}

#[test]
fn ctrl_c_quits() {
    let mut h = harness().started();
    h.press(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(h.app.quit);
    assert_eq!(h.app.busy, 0, "not compose");
}
