# dtu_tui

A terminal UI for file transfer on a LAN.
On start it hosts `dtu_core` in its own process and immediately becomes a receiver. Quit it and the services stop.

## Run

```bash
cargo build --release
./target/release/dtu-tui
```

## Screen

```
┌ dtu ─────────────────────────────────────────────────┐
│ 1 status  2 devices  3 invites  4 code      Scan LAN │
├──────────────────────────────────────────────────────┤
│ device   0tZr635ykNqC3_eYOsNnPQ                      │
│ version  0.1.0 (proto v1)                            │
│ port     47000                                       │
│ root     C:\Users\me\AppData\Roaming\dtu\share       │
│ trusted  2      pending 0      uptime 42s            │
├──────────────────────────────────────────────────────┤
│ 1-4 tab  r refresh  s scan  c new code  q quit       │
└──────────────────────────────────────────────────────┘
```

| Tab | Content | Keys there |
| --- | --- | --- |
| status | device id, version, listening port, root, counters, uptime | — |
| devices | trusted devices (alias, address, expiry; expired ones say `[expired]`) | — |
| invites | pending invitations | `↑↓` select, `a` accept, `d` deny |
| code | the one-time pairing code and its state | `c` rotate |

## Keys

| Key | Action |
| --- | --- |
| `1` … `4` / `tab` / `←` `→` | switch tab |
| `↑` `↓` | move the selection |
| `r` | reload now (on top of the 2 s auto refresh) |
| `s` | scan the LAN (waits 1.5 s) |
| `c` | rotate the pairing code |
| `a` / `d` | accept (30 days) / deny the selected invitation |
| `q` / `Esc` | quit |

## Pairing flow

1. Start `dtu-tui` on both machines
2. Press `s` on one side to see whether the peer answers (both must listen on the same port)
3. On the sending side run `dtu device add <host:port> --alias nas`
4. The invitation shows up under `invites` on the receiver — press `a`
5. From then on `dtu push nas ./file` works

An invitation carries only a device id and an address; no file data moves before you accept.

## Receiving

While the TUI is open it listens under `root` (default `<data dir>/share`).
A peer's `dtu push` writes straight into it: data lands as `<name>.dtu-part` and is renamed when complete.

## Notes

- One process owns the data directory (`DTU_HOME`, default `%APPDATA%\dtu`).
  Running `dtu status` while the TUI is open fails with `another dtu daemon already owns …`.
- Starting transfers is not the TUI's job — use `dtu push` / `pull` / `async` from the CLI, or `dtu-gui`.
- The REST surface for third parties runs at the same time on `127.0.0.1:<port+1>` (key in `control.token`).

## About this site

Built with MkDocs (Material theme + static-i18n) and published to GitHub Pages on every push to `main`.
Set **Settings → Pages → Source** to **GitHub Actions** once.

```bash
pip install -r requirements-docs.txt
mkdocs serve
```
