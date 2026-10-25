//! The TUI: one event loop fed by one channel.

pub mod app;
mod actions;
pub mod compose;
pub mod date;
pub mod jobs;
pub mod keys;
pub mod view;
#[cfg(test)]
mod app_tests;
#[cfg(test)]
pub mod testutil;

use crate::config::Config;
use app::{App, Effect, Msg};
use ratatui::{
    DefaultTerminal, Terminal,
    backend::CrosstermBackend,
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture},
        execute,
        terminal::{EnterAlternateScreen, enable_raw_mode},
    },
};
use std::{io, sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc::{self, Sender}}, thread, time::Duration};

pub fn run(cfg: Config) -> Result<(), String> {
    let (tx, rx) = mpsc::channel();
    let mut app = App::new(cfg, tx.clone())?;
    let paused = Arc::new(AtomicBool::new(false));
    spawn_input(tx.clone(), paused.clone());
    spawn_ticker(tx.clone());

    // One panic hook, installed once, ever: it disables mouse capture (ratatui's own
    // panic hook, installed below by try_init, doesn't know mouse capture is on — we
    // turned it on ourselves) and falls through to whatever hook was already there.
    // `ratatui::try_init()` installs its own wrapper around this on top (restoring raw
    // mode / the alt screen before calling us), which is fine since it only runs once;
    // what we must avoid is calling try_init/init again on every editor round trip,
    // since each call wraps yet another hook around the last — see `reenter_terminal`.
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(io::stdout(), DisableMouseCapture);
        previous_hook(info);
    }));

    let mut term = ratatui::try_init().map_err(|e| e.to_string())?;
    let _ = execute!(io::stdout(), EnableMouseCapture);
    app.start();
    let res = (|| -> Result<(), String> {
        loop {
            term.draw(|f| view::draw(f, &mut app)).map_err(|e| e.to_string())?;
            let first = rx.recv().map_err(|e| e.to_string())?;
            for msg in std::iter::once(first).chain(rx.try_iter()) {
                let mut effect = app.update(msg);
                while let Some(Effect::Edit(path)) = effect {
                    // Hand the terminal to $EDITOR; the input thread pauses so it doesn't eat keys.
                    paused.store(true, Ordering::SeqCst);
                    let _ = execute!(io::stdout(), DisableMouseCapture);
                    ratatui::restore();
                    let r = compose::edit(&path);
                    term = reenter_terminal().map_err(|e| e.to_string())?;
                    let _ = execute!(io::stdout(), EnableMouseCapture);
                    paused.store(false, Ordering::SeqCst);
                    app.edited(path, r);
                    effect = None;
                }
            }
            if app.quit {
                return Ok(());
            }
        }
    })();
    let _ = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    res
}

/// Re-enter raw mode and the alternate screen after $EDITOR exits. Deliberately *not*
/// `ratatui::init()`/`try_init()` again: both call their own panic-hook installer on every
/// invocation, so re-running one per editor round trip would wrap a new hook around the
/// last every time a message gets composed — an unbounded chain over a long session.
fn reenter_terminal() -> io::Result<DefaultTerminal> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(io::stdout()))
}

// ponytail: 50 ms poll; a key typed within 50 ms of the editor opening can be lost.
fn spawn_input(tx: Sender<Msg>, paused: Arc<AtomicBool>) {
    thread::spawn(move || loop {
        if paused.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(50));
            continue;
        }
        match event::poll(Duration::from_millis(50)) {
            Ok(true) => {
                if let Ok(ev) = event::read()
                    && tx.send(Msg::Input(ev)).is_err()
                {
                    return;
                }
            }
            Ok(false) => {}
            Err(_) => return,
        }
    });
}

fn spawn_ticker(tx: Sender<Msg>) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(100));
        if tx.send(Msg::Tick).is_err() {
            return;
        }
    });
}
