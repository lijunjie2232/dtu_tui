[![日本語](https://img.shields.io/badge/lang-日本語-868e96?style=flat-square)](README.md)
[![English](https://img.shields.io/badge/lang-English-1c7ed6?style=flat-square)](README.en.md)
[![繁體中文](https://img.shields.io/badge/lang-繁體中文-868e96?style=flat-square)](README.zh-TW.md)

# dtu_tui

A **terminal UI** for file transfer on a LAN.
It hosts `dtu_core` in its own process, so the moment it starts it is already a receiver. Close it and the services stop.

[Docs](docs/index.en.md) · [dtu_core](../dtu_core) · [dtu_cli](../dtu_cli) · [dtu_gui](../dtu_gui)

## Build and run

```bash
cargo build --release
./target/release/dtu-tui
```

## Tabs

| Tab | Content |
| --- | --- |
| status | device id, version, listening port, root, trust counters, uptime |
| devices | trusted devices (alias, address, expiry) |
| invites | pending invitations — **a** accepts, **d** denies |
| code | the one-time pairing code — **c** rotates it |
| log | who authenticated, and which files moved (peer, address, path, size) — **f** filters |
| settings | every key of config.json (ports, root, parallelism, trust, UI refresh…) editable in place |

## Keys

| Key | Action |
| --- | --- |
| `1` … `6` / `tab` / `←→` | switch tab |
| `↑` `↓` | move the selection |
| `r` | reload |
| `s` | scan the LAN |
| `c` | new pairing code |
| `f` | cycle the log filter (all / auth / files / invites) |
| `a` / `d` | accept / deny the selected invitation |
| `enter` | edit the selected setting (bools toggle in place) |
| `q` / `Esc` | quit (services stop too) |

It refreshes every two seconds. Files sent to you land in the `share` directory while the window stays open.

## Typical flow

1. Start `dtu-tui` on both machines
2. Press `s` on one side to see whether the other shows up
3. From the sending side, run `dtu device add <host:port>`
4. The invitation appears under `invites` on the receiver — press `a`
5. From then on `dtu push <alias> <file>` just works

## Notes

- One process owns the data directory (`DTU_HOME`, default `%APPDATA%\dtu`).
  Running `dtu status` while the TUI is open fails with `another dtu daemon already owns …`.
- Transfers themselves are not started from the TUI — use `dtu push` / `pull` / `async`, or the GUI.

## Docs

`docs/` is MkDocs (Material + static-i18n) in Japanese / English / 繁體中文.
`.github/workflows/docs.yml` publishes it to GitHub Pages on every push to `main`.

```bash
pip install -r requirements-docs.txt
mkdocs serve
```
