**Docs: <https://lubabs770.github.io/parcel/>**

# parcel

A light, fast, yummy terminal mail client for any email CLI.

- Drives any email CLI — himalaya ships built in, point it at another with a few command templates
- Starts instantly, no daemon, no index
- Live Omarchy theme: follows your system theme, repaints on change
- 8 built-in themes (and your own)
- Optimistic triage: star, archive, move, delete update the screen immediately, the backend call happens after
- Compose, reply, and forward hand off to `$EDITOR`
- Every TUI action is also a `--json` command, for scripts

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/lubabs770/parcel/main/install.sh | sh
```

Downloads the right binary for your OS/arch, verifies its sha256 against the
release's checksums, test-runs it, then installs to `~/.local/bin` (atomically —
a running `parcel` is never half-overwritten).

- `BIN_DIR=/some/dir ... | sh` to install somewhere else
- `VERSION=v0.1.0 ... | sh` to pin a release instead of latest

parcel needs an email CLI to drive. The default backend is
[himalaya](https://github.com/pimalaya/himalaya); install and configure that
(or another CLI — see Config below) first.

## The TUI

| Key | Action |
| --- | --- |
| `j` / `k` | down / up |
| `g` / `G` | top / bottom |
| `enter` | open |
| `esc` | back |
| `tab` | next pane |
| `/` | search |
| `r` / `R` | reply / reply all |
| `f` | forward |
| `c` | compose |
| `s` | star |
| `u` | toggle read |
| `d` | delete |
| `m` | move to… |
| `a` | archive |
| `ctrl+p` | command palette |
| `t` | themes |
| `?` | help |
| `q` | quit |

Arrow keys, Home and End always move, regardless of overrides. Mouse works
too: scroll a pane, click to select, double-click to open.

Every key is rebindable — see Config below.

## Config

```
~/.config/parcel/
├── config.toml           # backend, theme, page_size, inbox, archive
├── keys.toml             # key overrides
└── backends/
    └── himalaya.toml     # command templates for the himalaya backend
```

`parcel config init` writes the defaults above for editing (never overwrites
an existing file). `backends/himalaya.toml` is written fully commented out:
uncomment only the lines you want to override, so the rest keeps following the
built-in preset. `parcel config path` prints the config directory.

The himalaya preset is built in — no file needed until you want to change it.
Every key in `config.toml` is optional; the defaults are `backend = "himalaya"`,
`theme = "auto"`, `page_size = 50`, `inbox = "inbox"`, `archive = "Archive"`.

To drive another CLI, set `backend` and write `backends/<name>.toml`: a command
template per action, with `{mailbox} {id} {page} {size} {query} {flag}
{target}` filled in (and `{name...}` to split a value on whitespace), plus a
`[fields]` table mapping its JSON output to parcel's envelope shape:

```toml
# ~/.config/parcel/config.toml
backend = "mycli"

# ~/.config/parcel/backends/mycli.toml
mailboxes = "mycli folders --json"
list      = "mycli ls --folder {mailbox} --page {page} --json"
read      = "mycli show {id}"
[fields]
items = ""           # output is a bare array
id = "uid"
subject = "subject"
from = "sender"
date = "date"
flags = "flags"
mailbox_items = ""
mailbox_name = "name"
```

Leave an action's template out to disable its key.

## Omarchy

With `theme = "auto"` (the default), parcel follows the active
[Omarchy](https://omarchy.org) theme and re-derives any color layer Omarchy's
palette doesn't define.

`parcel omarchy hook install` adds a line to Omarchy's `theme-set` hook that
signals running parcels to repaint instantly on a theme change. Without it,
parcel still notices within about a second via polling — the hook just makes
it immediate. `parcel omarchy hook uninstall` removes the line.

## CLI

```
parcel                 open the TUI
parcel mailboxes        list mailboxes
parcel list              list envelopes in a mailbox
parcel search QUERY       search a mailbox
parcel read ID              print a message
parcel send [FILE]           send a raw RFC 5322 message (FILE or stdin)
parcel config init|path       write/print the config files
parcel omarchy hook install|uninstall
parcel doctor                 check the backend, templates and theme setup
parcel log                     show the end of the error log
parcel upgrade [--check]        install the latest release (or just check)
```

Add `--json` to any data-producing command for machine-readable output.

`parcel doctor` checks that every configured backend command is on `$PATH`,
that mailboxes actually load, and whether the Omarchy hook is installed.

`parcel log` shows the end of parcel's error log — every failed backend call
is recorded there with its command and stderr, even when the TUI only shows a
short status-bar message.

## Troubleshooting

**himalaya can't reach the macOS keychain.** This happens when parcel is
launched from a session without keychain access — a detached tmux or a
headless/SSH session, for example. The backend error shows up in the status
bar; run `parcel log` for the full command and stderr.
