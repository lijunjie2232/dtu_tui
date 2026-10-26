# dtu_tui

LAN 内のファイル転送を端末から操作する TUI アプリです。
起動すると `dtu_core` を自分のプロセスにホストし、そのまま受信側として待ち受けます。終了すればサービスも止まります。

## 起動

```bash
cargo build --release
./target/release/dtu-tui
```

## 画面

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

| タブ | 内容 | そこで使えるキー |
| --- | --- | --- |
| status | デバイス ID、バージョン、待ち受けポート、ルート、信頼数、稼働時間 | — |
| devices | 信頼済みデバイス（別名・アドレス・有効期限、期限切れは `[expired]`） | — |
| invites | 承認待ちの招待 | `↑↓` 選択、`a` 承認、`d` 拒否 |
| code | ワンタイムのペアリングコードと状態 | `c` 再生成 |
| log | 認証と、実際に動いたファイル（相手 ID・IP・パス・サイズ） | `f` で絞り込み（all / auth / files / invites） |
| settings | config.json の全項目（ポート、ルート、並列、信頼まわり、UI 更新間隔…） | `↑↓` 選択、`enter` で編集（bool は切替） |

## キー

| キー | 動作 |
| --- | --- |
| `1` … `6` / `tab` / `←` `→` | タブ切り替え |
| `↑` `↓` | 一覧の選択 |
| `r` | 再読み込み（2 秒ごとの自動更新とは別に即時反映） |
| `s` | LAN を探索（1.5 秒待つ） |
| `c` | ペアリングコードを再生成 |
| `a` / `d` | 選択中の招待を承認（30 日） / 拒否 |
| `f` | log タブの表示を切り替え（all → auth → files → invites） |
| `enter` | settings タブで編集を開始（bool はその場で切替） |
| `q` / `Esc` | 終了 |

## ペアリングの流れ

1. 両方の端末で `dtu-tui` を起動
2. 片方で `s` を押し、相手が見えるか確認（同じポートで待ち受けている必要があります）
3. 送る側で `dtu device add <host:port> --alias nas`
4. 受け取る側の `invites` タブに招待が出るので `a` で承認
5. 以降は `dtu push nas ./file` で送れる

招待はデバイス ID とアドレスだけを持ちます。中身（ファイル）は承認後にしか流れません。

## 設定（settings タブ）

`config.json` の全項目をその場で編集できます。行は `key / 現在値 / 反映時期 / 説明` の順。
`live` の項目（`code_password`）は保存と同時に動いているリスナーへ反映、`next start` の項目（ポート・ルート・各種タイムアウトなど）は次回起動からです。

| キー | 既定 | 内容 |
| --- | --- | --- |
| `port` | 47000 | 待ち受け／探索のポート |
| `api_port` | 0 | REST の loopback ポート（0 = port + 1） |
| `root` | `<データディレクトリ>/share` | 相手に見せるディレクトリ |
| `parallel` | 4 | 1 ファイルを何本の TCP で送るか |
| `unlimit` | false | 危険：全接続を受け入れる |
| `code_password` | （なし） | コードに追加するパスワード（空で解除） |
| `ui_close_to_tray` | true | GUI 用：ウィンドウを閉じたらトレイに残す |

`dtu config fields`（CLI）で全項目の一覧と説明を確認できます。

## 受け取り

TUI を開いている間は `root`（既定は `<データディレクトリ>/share`）で待ち受けています。
相手が `dtu push` すれば、そのまま書き込まれます。書き込み中は `<name>.dtu-part` で、
完了してから本来の名前になります。

## 注意

- 1 つのデータディレクトリ（`DTU_HOME`、既定 `%APPDATA%\dtu`）は 1 アプリ専有。
  TUI を開いたまま `dtu status` を実行すると `another dtu daemon already owns …` になります。
- 転送の開始は TUI の仕事ではありません。`dtu push` / `pull` / `async`（CLI）か `dtu-gui` を使ってください。
- 第三者向けの REST 面は `127.0.0.1:<port+1>` で同時に動いています（鍵は `control.token`）。

## このサイトについて

MkDocs（Material + static-i18n）で生成し、`main` への push で GitHub Pages に公開します。
初回だけ **Settings → Pages → Source** を **GitHub Actions** に設定してください。

```bash
pip install -r requirements-docs.txt
mkdocs serve
```

