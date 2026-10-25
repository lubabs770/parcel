//! A real App wired to tests/fake-mail.sh: backend jobs run for real, offline.

use super::app::{App, Effect, Msg};
use crate::config::Config;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use std::{fs, sync::{Once, mpsc}, time::Duration};

pub struct Harness {
    pub app: App,
    pub rx: mpsc::Receiver<Msg>,
    pub dir: tempfile::TempDir,
}

pub fn harness() -> Harness {
    build("send")
}

/// Like `harness()`, but the `send` template runs the fake CLI's `fail` case instead of
/// its `send` one — for a test that needs a send which fails.
pub fn harness_with_failing_send() -> Harness {
    build("fail")
}

fn build(send_action: &str) -> Harness {
    static ENV: Once = Once::new();
    // SAFETY: set once, before any test reads it; keeps parcel.log and drafts out of $HOME.
    ENV.call_once(|| unsafe { std::env::set_var("PARCEL_STATE_DIR", std::env::temp_dir().join("parcel-test-state")) });
    let dir = tempfile::tempdir().unwrap();
    let sh = format!("sh {}/tests/fake-mail.sh {}", env!("CARGO_MANIFEST_DIR"), dir.path().join("argv.log").display());
    fs::create_dir(dir.path().join("backends")).unwrap();
    fs::write(dir.path().join("config.toml"), "backend = \"fake\"\ntheme = \"tokyo-night\"\npage_size = 25\n").unwrap();
    fs::write(
        dir.path().join("backends/fake.toml"),
        format!(
            r#"mailboxes = "{sh} mailboxes"
list = "{sh} list {{mailbox}} {{page}} {{size}}"
search = "{sh} list {{mailbox}} {{query}}"
read = "{sh} read {{id}}"
flag = "{sh} flag {{id}} {{flag}}"
unflag = "{sh} unflag {{id}} {{flag}}"
move = "{sh} move {{id}} {{target}}"
delete = "{sh} fail {{id}}"
send = "{sh} {send_action}"
reply = "{sh} reply {{id}}"
seen_flag = "seen"
flagged_flag = "flagged"
[fields]
items = "items"
id = "id"
subject = "subject"
from = "from.name|from.email"
date = "date"
flags = "flags"
flag_name = "name"
mailbox_items = "boxes"
mailbox_name = "name"
"#
        ),
    )
    .unwrap();
    let cfg = Config::load_from(dir.path()).unwrap();
    let (tx, rx) = mpsc::channel();
    Harness { app: App::new(cfg, tx).unwrap(), rx, dir }
}

impl Harness {
    /// Feed the next finished backend job back into the app.
    pub fn settle(&mut self) -> Option<Effect> {
        let msg = self.rx.recv_timeout(Duration::from_secs(5)).expect("a backend job finished");
        self.app.update(msg)
    }

    pub fn press(&mut self, code: KeyCode, mods: KeyModifiers) -> Option<Effect> {
        self.app.update(Msg::Input(Event::Key(KeyEvent::new(code, mods))))
    }

    pub fn type_str(&mut self, s: &str) {
        for c in s.chars() {
            self.press(KeyCode::Char(c), KeyModifiers::NONE);
        }
    }

    /// start() and settle both initial jobs (mailboxes + first page).
    pub fn started(mut self) -> Self {
        self.app.start();
        self.settle();
        self.settle();
        self
    }
}
