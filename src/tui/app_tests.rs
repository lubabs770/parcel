use super::{app::*, keys::Action, testutil::{harness, harness_with_failing_send}};
use crate::backend::Envelope;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

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
