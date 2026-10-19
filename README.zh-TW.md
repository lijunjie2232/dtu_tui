[![日本語](https://img.shields.io/badge/lang-日本語-868e96?style=flat-square)](README.md)
[![English](https://img.shields.io/badge/lang-English-868e96?style=flat-square)](README.en.md)
[![繁體中文](https://img.shields.io/badge/lang-繁體中文-1c7ed6?style=flat-square)](README.zh-TW.md)

# dtu_tui

用終端機操作 LAN 檔案傳輸的 **TUI 應用**。
它把 `dtu_core` 載入自己的行程，所以一啟動就是「收檔端」；關掉它，服務也一起停。

[文件](docs/index.zh-TW.md) · [dtu_core](../dtu_core) · [dtu_cli](../dtu_cli) · [dtu_gui](../dtu_gui)

## 建置與啟動

```bash
cargo build --release
./target/release/dtu-tui
```

## 畫面

共有四個分頁。

| 分頁 | 內容 |
| --- | --- |
| status | 裝置 ID、版本、監聽埠、根目錄、信任數、運行時間 |
| devices | 已信任裝置（別名、位址、到期時間） |
| invites | 待批准邀請；**a** 批准、**d** 拒絕 |
| code | 一次性配對碼；**c** 重新產生 |

## 按鍵

| 按鍵 | 動作 |
| --- | --- |
| `1` … `4` / `tab` / `←→` | 切換分頁 |
| `↑` `↓` | 移動選取 |
| `r` | 重新讀取 |
| `s` | 掃描區網 |
| `c` | 重新產生配對碼 |
| `a` / `d` | 批准 / 拒絕選取的邀請 |
| `q` / `Esc` | 離開（服務也停止） |

每兩秒自動更新。別人傳過來的檔案會直接進到 `share` 目錄。

## 典型流程

1. 兩台機器都啟動 `dtu-tui`
2. 在其中一邊按 `s`，確認看得到對方
3. 送檔方用 CLI 執行 `dtu device add <host:port>`
4. 收檔方的 `invites` 分頁會出現邀請，按 `a` 批准
5. 之後就能 `dtu push <alias> <file>`

## 注意

- 一個資料目錄（`DTU_HOME`，預設 `%APPDATA%\dtu`）只能有一個應用。
  TUI 開著時執行 `dtu status` 會得到 `another dtu daemon already owns …`。
- TUI 本身不發起傳輸，請用 `dtu push` / `pull` / `async` 或 GUI。

## 文件

`docs/` 用 MkDocs（Material + static-i18n），三種語言：日本語 / English / 繁體中文。
`.github/workflows/docs.yml` 會在 push 到 `main` 時發佈到 GitHub Pages。

```bash
pip install -r requirements-docs.txt
mkdocs serve
```
