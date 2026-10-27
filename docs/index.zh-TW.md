# dtu_tui

用終端機操作 LAN 檔案傳輸的 TUI 應用。
啟動時把 `dtu_core` 載入自己的行程，立刻就是收檔端；關掉它，服務也一起停止。

## 啟動

```bash
cargo build --release
./target/release/dtu-tui
```

## 畫面

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

| 分頁 | 內容 | 可用按鍵 |
| --- | --- | --- |
| status | 裝置 ID、版本、監聽埠、根目錄、計數、運行時間 | — |
| devices | 已信任裝置（別名、位址、到期時間；過期顯示 `[expired]`） | — |
| invites | 待批准邀請 | `↑↓` 選取、`a` 批准、`d` 拒絕 |
| code | 一次性配對碼與狀態 | `c` 重新產生 |
| log | 誰通過認證、以及真正搬動過的檔案（對端、位址、路徑、大小） | `f` 切換篩選（all / auth / files / invites） |
| settings | config.json 的全部鍵（埠、根目錄、平行、信任、UI 更新間隔…） | `↑↓` 選取、`enter` 編輯（bool 直接切換） |

## 按鍵

| 按鍵 | 動作 |
| --- | --- |
| `1` … `6` / `tab` / `←` `→` | 切換分頁 |
| `↑` `↓` | 移動選取 |
| `r` | 立即重新讀取（另有每 2 秒自動更新） |
| `s` | 掃描區網（等 1.5 秒） |
| `c` | 重新產生配對碼 |
| `a` / `d` | 批准（30 天） / 拒絕選取的邀請 |
| `f` | 循環 log 分頁的篩選（all → auth → files → invites） |
| `enter` | settings 分頁開始編輯（bool 直接切換） |
| `q` / `Esc` | 離開 |

## 配對流程

1. 兩台機器都啟動 `dtu-tui`
2. 在其中一邊按 `s`，看對方是否有回應（雙方要監聽同一個埠）
3. 送檔方執行 `dtu device add <host:port> --alias nas`
4. 收檔方的 `invites` 分頁會出現邀請，按 `a` 批准
5. 之後 `dtu push nas ./file` 就能用

邀請只帶裝置 ID 與位址，批准之前不會有任何檔案內容流動。

## 設定（settings 分頁）

`config.json` 的每個鍵都能就地編輯，每列是 `key / 目前值 / 生效時機 / 說明`。
`live`（`code_password`）存檔後立即套用到正在運行的監聽端；`next start`（埠、根目錄、各種逾時…）要等下次啟動。

| 鍵 | 預設 | 說明 |
| --- | --- | --- |
| `port` | 47000 | 監聽與探索的埠 |
| `api_port` | 0 | REST 的 loopback 埠（0 = port + 1） |
| `root` | `<資料目錄>/share` | 對外分享的目錄 |
| `parallel` | 4 | 每個檔案用幾條 TCP |
| `unlimit` | false | 危險：接受任何裝置 |
| `code_password` | 無 | 配對碼的額外密碼（清空即解除） |
| `ui_close_to_tray` | true | 僅 GUI：關掉視窗就縮到系統匣 |

`dtu config fields`（CLI）可列出同樣的說明。

## 接收

TUI 開著的時候會在 `root`（預設 `<資料目錄>/share`）監聽。
對方 `dtu push` 過來的資料會先寫成 `<name>.dtu-part`，完成後才改成正式檔名。

## 注意

- 一個資料目錄（`DTU_HOME`，預設 `%APPDATA%\dtu`）只能有一個行程。
  TUI 開著時執行 `dtu status` 會得到 `another dtu daemon already owns …`。
- 發起傳輸不是 TUI 的工作，請用 CLI 的 `dtu push` / `pull` / `async` 或 `dtu-gui`。
- 給第三方用的 REST 介面同時在 `127.0.0.1:<port+1>` 上運行（金鑰在 `control.token`）。

## 關於本網站

用 MkDocs（Material 主題 + static-i18n）產生，push 到 `main` 時自動發佈到 GitHub Pages。
第一次請把 **Settings → Pages → Source** 設成 **GitHub Actions**。

```bash
pip install -r requirements-docs.txt
mkdocs serve
```
