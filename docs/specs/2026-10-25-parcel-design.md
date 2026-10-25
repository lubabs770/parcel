# parcel — design

A light, fast, good-looking terminal mail client that drives any email CLI.
Himalaya is the built-in default. Sibling of
[gooseneck](https://github.com/lubabs770/gooseneck): same feel, same "CLI can
do everything the TUI can" rule.

## Goals

- Read, triage (star / read / move / archive / delete) and compose mail from one TUI.
- Work with any email CLI that prints JSON, by config alone.
- Feel instant: startup < 10 ms, UI never blocks on the backend.
- Follow the Omarchy theme live; look great everywhere else too.
- Every config file stays small and single-purpose.

## Non-goals (v1)

Multi-account switcher, attachments, threading, HTML rendering, disk cache,
notmuch/mblaze presets (config-only, add when someone uses them).

## Stack

Rust. `ratatui` + `crossterm` (TUI), `clap` (CLI), `serde`, `serde_json`,
`toml`. Release profile: `lto = true`, `strip = true`, `opt-level = 3`,
`codegen-units = 1`. Target binary ~3 MB.

Builds and releases run on GitHub Actions (macOS arm64/x86_64, Linux
x86_64/arm64 musl). `install.sh` installs `parcel` to `~/.local/bin`, like
gooseneck's.

## Source layout

Every source file stays under ~300 lines.

```
src/
  main.rs            clap entry; dispatches to cli:: or tui::run
  config/            load + merge: config.toml, keys.toml, backends/, themes/
  backend/
    template.rs      "{placeholder}" expansion -> argv (no shell)
    fields.rs        dot-path + "|" fallback extraction from serde_json::Value
    exec.rs          run a command, capture stdout/stderr, map errors
    mod.rs           Backend: list/search/read/mailboxes/flag/move/delete/send/reply_template
  theme/
    builtin.rs       ~8 compiled-in themes
    omarchy.rs       detect + map colors.toml / alacritty.toml; signature for polling
  tui/
    app.rs           state, event loop, worker channel
    mailboxes.rs     left pane
    list.rs          envelope list pane
    reader.rs        message pane
    palette.rs       ctrl+p command palette + fuzzy mailbox picker
    themes.rs        live theme picker
    status.rs        status bar, spinner, errors
    compose.rs       $EDITOR round-trip + send/edit/discard prompt
    keys.rs          default keymap + keys.toml overrides
  cli/               list, read, search, send, mailboxes, doctor, log, config init, omarchy hook
```

## Configuration

A directory with one concern per small file. Every file is optional: the
himalaya preset, default keymap and built-in themes are compiled in, and files
on disk override them key by key. `parcel config init` writes the defaults out.

```
~/.config/parcel/          ($XDG_CONFIG_HOME/parcel)
  config.toml              backend, theme, page_size  (5–10 lines)
  keys.toml                only overrides
  backends/<name>.toml     action templates + field map (~15 lines)
  themes/<name>.toml       12 colour tokens
```

`config.toml`:

```toml
backend   = "himalaya"
theme     = "auto"        # "auto" = Omarchy if detected, else tokyo-night
page_size = 50
```

`backends/himalaya.toml` (compiled-in default, targets himalaya 2.x):

```toml
mailboxes      = "himalaya --json mailbox list"
list           = "himalaya --json envelope list -m {mailbox} --page {page} --page-size {size}"
search         = "himalaya --json envelope list -m {mailbox} {query}"
read           = "himalaya message read -m {mailbox} {id}"
flag           = "himalaya flag add -m {mailbox} {id} {flag}"
unflag         = "himalaya flag remove -m {mailbox} {id} {flag}"
move           = "himalaya message move -m {mailbox} {target} {id}"
delete         = "himalaya message delete -m {mailbox} {id}"
send           = "himalaya message send"            # RFC 5322 on stdin
reply_template = "himalaya template reply -m {mailbox} {id}"
forward_template = "himalaya template forward -m {mailbox} {id}"

[fields]
items   = "envelopes"
id      = "id"
subject = "subject"
from    = "from.0.name|from.0.email"
date    = "date"
flags   = "flags"
mailbox_items = "mailboxes"
mailbox_name  = "name"
```

Exact himalaya 2.x subcommand flags are verified against `himalaya --help`
during implementation; the shape above is the contract.

Rules:

- Templates are split on whitespace first, then each `{placeholder}` is
  substituted inside its own argv entry and run with `std::process::Command`.
  No shell, so subjects/queries cannot inject.
- `read` output is plain text, shown as-is (so any CLI's reader works).
- A missing template disables that action; its key renders dim.
- `{query}` for search is passed as one argv entry; backends that need it split
  can use `{query...}` (split on whitespace).

`themes/<name>.toml` tokens: `bg surface overlay fg muted accent unread flagged
danger ok selection border`.

## Theming

- Built-in: tokyo-night, catppuccin, gruvbox, rose-pine, nord, everforest,
  kanagawa, and one light theme.
- **Omarchy** (ported from gooseneck `omarchy.go`): look in
  `$XDG_STATE_HOME/omarchy/current/theme` (Omarchy 4), then
  `$XDG_CONFIG_HOME/omarchy/current/theme` (Omarchy 3); override with
  `PARCEL_OMARCHY_THEME`. Read `colors.toml`, fall back to `alacritty.toml`.
  Light/dark from `mode` or background luminance; derive missing surface layers
  by nudging the background.
- **Live follow, two ways:**
  - `parcel omarchy hook install` appends `pkill -USR1 -x parcel` to
    `~/.config/omarchy/hooks/theme-set` (idempotent; `uninstall` removes the
    line). Running parcels repaint on SIGUSR1 in the same frame.
  - Without the hook: a 1 s stat poll of the theme dir (path + mtimes + sizes)
    triggers the same repaint.
- **Look:** painted layered surfaces instead of box lines, accent unread dot,
  flag glyph, sender-initial avatar chip, relative dates ("2h", "Tue", "Oct 3"),
  status-bar spinner while a command runs, `t` = live theme picker with preview.

## TUI

Three panes (mailboxes · list · reader); below 100 columns it collapses to a
single pane stack with `esc` going back.

| Key | Action |
|---|---|
| `j`/`k`, `g`/`G` | move, top/bottom |
| `enter` / `esc` / `tab` | open / back / next pane |
| `/` | search (backend `search` template) |
| `r` `R` `f` `c` | reply, reply-all, forward, compose |
| `s` `u` `d` `m` `a` | star, toggle read, delete, move (fuzzy picker), archive |
| `ctrl+p` `t` `?` `q` | palette, themes, help, quit |

Mouse: click selects, double-click opens, wheel scrolls.

Compose: get the reply/forward template from the backend (or a blank header
block for `c`), write it to a temp file, open `$EDITOR` (fallback `vi`), then
prompt `send · edit · discard`. Send pipes the file to the `send` template.
The temp file is kept until send succeeds, so a failed send never loses a draft.

## Data flow

- Backend calls run on a worker thread pool; results come back to the event
  loop over an `mpsc` channel. Rendering never waits on a command.
- Triage is optimistic: the row updates immediately and reverts with a status
  error if the command fails.
- Next page prefetches when the cursor is within 10 rows of the end.
- Message bodies: in-memory LRU of 50. No disk cache.

## Errors

- Non-zero exit: first stderr line in the status bar; full text appended to
  `$XDG_STATE_HOME/parcel/parcel.log` (`parcel log` tails it).
- Unparseable JSON: status error naming the template and the field path that
  failed.
- `parcel doctor`: backend binary on PATH, each template's binary resolvable,
  `mailboxes` runs and parses, Omarchy detected or not, hook installed or not.

## CLI

`parcel list|search|read|send|mailboxes|doctor|log|config init|omarchy hook install|uninstall`,
each with `--json` where it produces data. `parcel` with no args opens the TUI.

## Testing

- Unit: template expansion (incl. hostile input stays one argv entry), field
  paths and `|` fallback, Omarchy `colors.toml`/`alacritty.toml` mapping,
  keymap override merge.
- Integration: backend pointed at `tests/fake-mail.sh`, which prints canned
  JSON; covers list → read → flag → move → send.
- Render: ratatui `TestBackend` snapshot of the main screen at 120×30 and 80×24.
