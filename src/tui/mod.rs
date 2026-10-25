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
use ratatui::crossterm::{event::{self, DisableMouseCapture, EnableMouseCapture}, execute};
use std::{io, sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc::{self, Sender}}, thread, time::Duration};

pub fn run(cfg: Config) -> Result<(), String> {
    let (tx, rx) = mpsc::channel();
    let mut app = App::new(cfg, tx.clone())?;
    let paused = Arc::new(AtomicBool::new(false));
    spawn_input(tx.clone(), paused.clone());
    spawn_ticker(tx.clone());
    let mut term = ratatui::init();
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
                    term = ratatui::init();
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
