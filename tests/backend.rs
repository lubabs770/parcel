use parcel::backend::{Backend, Draft, Flag};
use parcel::config::{BackendSpec, Fields};
use std::fs;

#[test]
fn round_trip_through_a_fake_cli() {
    let state = tempfile::tempdir().unwrap();
    // SAFETY: the only test in this binary; nothing else reads the env concurrently.
    unsafe { std::env::set_var("PARCEL_STATE_DIR", state.path()) }
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("argv.log");
    let sh = format!("sh {}/tests/fake-mail.sh {}", env!("CARGO_MANIFEST_DIR"), log.display());
    let t = |rest: &str| Some(format!("{sh} {rest}"));
    let be = Backend::new(
        BackendSpec {
            mailboxes: t("mailboxes"),
            list: t("list {mailbox} {page} {size}"),
            read: t("read {id}"),
            flag: t("flag {id} {flag}"),
            unflag: t("unflag {id} {flag}"),
            move_: t("move {id} {target}"),
            delete: t("fail {id}"),
            send: t("send"),
            reply: t("reply {id}"),
            seen_flag: "seen".into(),
            flagged_flag: "flagged".into(),
            fields: Fields {
                items: "items".into(), id: "id".into(), subject: "subject".into(),
                from: "from.name|from.email".into(), date: "date".into(), flags: "flags".into(),
                flag_name: "name".into(), mailbox_items: "boxes".into(), mailbox_name: "name".into(),
            },
            ..Default::default()
        },
        25,
    );

    assert_eq!(be.mailboxes().unwrap(), ["Inbox", "Archive"]);
    let list = be.list("Inbox", 1).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!((list[0].from.as_str(), list[0].seen, list[0].flagged), ("Ada", true, false));
    assert_eq!((list[1].from.as_str(), list[1].seen, list[1].flagged), ("bob@x.io", false, true));
    assert!(be.read("Inbox", "1").unwrap().contains("Hi there"));
    be.set_flag("Inbox", "2", Flag::Flagged, false).unwrap();
    be.move_to("Inbox", "1", "Archive").unwrap();
    be.send("To: ada@x.io\n\nhello\n").unwrap();
    assert!(be.draft(Draft::Reply, "Inbox", "1").unwrap().starts_with("Subject: Re:"));
    assert!(be.draft(Draft::New, "Inbox", "").unwrap().starts_with("To:"), "no compose template → blank headers");
    assert!(be.draft(Draft::ReplyAll, "Inbox", "1").unwrap_err().contains("no `reply_all` template"));
    assert_eq!(be.delete("Inbox", "1").unwrap_err(), "delete: boom: mailbox gone");
    assert!(fs::read_to_string(state.path().join("parcel.log")).unwrap().contains("boom"));

    let calls = fs::read_to_string(log).unwrap();
    assert!(calls.contains("list Inbox 1 25"));
    assert!(calls.contains("unflag 2 flagged"));
    assert!(calls.contains("move 1 Archive"));
    assert!(calls.contains("hello"), "send got the message on stdin");
}
