[![日本語](https://img.shields.io/badge/lang-日本語-1c7ed6?style=flat-square)](README.md)
[![English](https://img.shields.io/badge/lang-English-868e96?style=flat-square)](README.en.md)
[![繁體中文](https://img.shields.io/badge/lang-繁體中文-868e96?style=flat-square)](README.zh-TW.md)

# dtu_tui

LAN 内のファイル転送を端末から操作する **TUI アプリ**です。
`dtu_core` を自分のプロセスにホストするので、起動した時点で「受け取る側」として待ち受けます。閉じればサービスも止まります。

[ドキュメント](docs/index.md) · [dtu_core](../dtu_core) · [dtu_cli](../dtu_cli) · [dtu_gui](../dtu_gui)

## ビルドと起動

```bash
cargo build --release
./target/release/dtu-tui
```

## 画面

5 つのタブがあります。

| タブ | 内容 |
| --- | --- |
| status | デバイス ID、バージョン、待ち受けポート、ルート、信頼数、稼働時間 |
| devices | 信頼済みデバイス（別名・アドレス・有効期限） |
| invites | 承認待ちの招待。**a** で承認、**d** で拒否 |
| code | ワンタイムのペアリングコード。**c** で再生成 |
| log | 認証と、実際に動いたファイル（相手 ID・IP・パス・サイズ）。**f** で絞り込み |
| settings | config.json の全項目（ポート、ルート、並列数、信頼まわり、UI 更新間隔…）をその場で編集 |

## キー

| キー | 動作 |
| --- | --- |
| `1` … `6` / `tab` / `←→` | タブ切り替え |
| `↑` `↓` | 一覧の選択 |
| `r` | 再読み込み |
| `s` | LAN を探索 |
| `c` | ペアリングコードを再生成 |
| `f` | log の絞り込み（all / auth / files / invites） |
| `a` / `d` | 選択中の招待を承認 / 拒否 |
| `enter` | settings タブで編集開始（bool はその場で切替） |
| `q` / `Esc` | 終了（サービスも停止） |

2 秒ごとに自動更新します。誰かが転送してくると、そのまま `share` ディレクトリに届きます。

## 典型的な流れ

1. 両方の端末で `dtu-tui` を起動する
2. 片方で `s` を押して相手が見えるか確認する
3. 送る側は CLI で `dtu device add <host:port>` を実行
4. 受け取る側の TUI の `invites` タブに招待が出るので `a` で承認
5. 以降は `dtu push <alias> <file>` で送れる

## 注意

- 1 つのデータディレクトリ（`DTU_HOME`、既定 `%APPDATA%\dtu`）は 1 アプリ専有です。
  TUI を開いたまま `dtu status` を実行すると `another dtu daemon already owns …` になります。
- 転送そのものは TUI からは行いません（`dtu push` / `pull` / `async` か GUI を使ってください）。

## ドキュメント

`docs/` は MkDocs（Material + static-i18n）で、日本語 / English / 繁體中文。
`.github/workflows/docs.yml` が `main` への push で GitHub Pages に公開します。

```bash
pip install -r requirements-docs.txt
mkdocs serve
```
