use super::{app::*, keys::Action, testutil::harness};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

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
fn stale_list_results_are_dropped() {
    let mut h = harness();
    h.app.mailbox = "Archive".into();
    h.app.update(Msg::Done(Done::List { mailbox: "Inbox".into(), query: None, page: 1, res: Ok(vec![]) }));
    assert!(h.app.loading || h.app.envelopes.is_empty());
    assert_eq!(h.app.page, 1);
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
