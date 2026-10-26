//! dtu terminal UI. It *is* the app: the core (listener, discovery, REST surface)
//! starts with it and stops when it exits, so nothing else has to be running.
//!
//! Keys: 1-4/tab switch, up/down select, r refresh, s scan LAN, c new code,
//! a accept invite, d deny invite, q quit.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use dtu_core::proto::InviteView;
use dtu_core::state::{Decision, State};
use dtu_core::trust::Store;
use dtu_core::{cfg, discover, embed, guard, identity, invite, proto};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs};
use ratatui::Terminal;

const TABS: [&str; 6] = ["status", "devices", "invites", "code", "log", "settings"];
/// What the log tab shows; `None` is everything.
const FILTERS: [(&str, Option<&str>); 4] =
    [("all", None), ("auth", Some("auth")), ("files", Some("file")), ("invites", Some("invite"))];

#[derive(Default, Clone)]
struct View {
    status: Vec<String>,
    devices: Vec<String>,
    invites: Vec<InviteView>,
    code: Vec<String>,
    /// Audit log: who authenticated, and which files moved.
    logs: Vec<dtu_core::audit::Entry>,
    /// Every setting as (key, current value), straight from config.json.
    settings: Vec<(String, String)>,
    /// Last action result.
    note: String,
}

enum Cmd {
    Refresh,
    Scan,
    NewCode,
    Accept(String),
    Deny(String),
    SetConfig { key: String, value: String },
}

/// Read everything the screen shows straight out of the live core state.
async fn snapshot(st: &Arc<State>, log_lines: usize) -> View {
    let mut v = View {
        status: vec![
            format!("device   {}", st.id.id),
            format!("version  {} (proto v{})", dtu_core::VERSION, proto::PROTO),
            format!("port     {}", st.cfg.port),
            format!("root     {}", guard::for_display(&st.root)),
            format!("unlimit  {}", st.cfg.unlimit),
            format!("trusted  {}", st.trust.lock().await.devices.len()),
            format!("pending  {}", st.invites.lock().await.len()),
            format!("uptime   {}s", identity::now().saturating_sub(st.started)),
        ],
        ..Default::default()
    };

    let store: Store = st.trust.lock().await.clone();
    for (id, d) in &store.devices {
        let expires = if d.expires_at == 0 {
            "never".to_string()
        } else {
            dtu_core::audit::iso(d.expires_at)
        };
        let live = if store.get(id).is_some() { "" } else { " [expired]" };
        v.devices.push(format!(
            "{id}  {}  {}  expires {expires}{live}",
            d.alias.as_deref().unwrap_or("-"),
            d.addr.clone().unwrap_or_else(|| "-".into())
        ));
    }

    v.invites = invite::list(st).await;
    let c = st.code.lock().await;
    v.code = vec![
        format!("code     {}", c.code),
        format!("password {}", if c.password.is_some() { "required" } else { "not set" }),
        format!("state    {}", if c.consumed { "consumed (refresh to reuse)" } else { "fresh" }),
    ];
    v.logs = dtu_core::audit::entries(None, log_lines);
    let cfg = cfg::Config::load();
    v.settings = cfg::fields()
        .iter()
        .map(|f| (f.key.to_string(), cfg.get(f.key).unwrap_or_default()))
        .collect();
    v
}

/// One user action, applied to the core in this process.
async fn act(st: &Arc<State>, cmd: Cmd) -> Result<String, String> {
    match cmd {
        Cmd::Refresh => Ok(String::new()),
        Cmd::Scan => {
            let trust = st.trust.lock().await.clone();
            let items = discover::scan(st.cfg.port, 1500, &st.id.id, &trust).await;
            let mut out = format!("{} device(s) answered", items.len());
            for s in items {
                out.push_str(&format!(
                    "\n{} {} {}",
                    s.id,
                    s.addr,
                    if s.trusted { "trusted" } else { "" }
                ));
            }
            Ok(out)
        }
        Cmd::NewCode => {
            st.rotate_code(None).await;
            let c = st.code.lock().await;
            Ok(format!("new code {}", c.code))
        }
        Cmd::Accept(id) => {
            let expires_at = cfg::parse_expiry("30d")?;
            invite::decide(st, &id, Decision::Accept { expires_at }).await?;
            Ok(format!("accepted invite {id}"))
        }
        Cmd::Deny(id) => {
            invite::decide(st, &id, Decision::Deny).await?;
            Ok(format!("denied invite {id}"))
        }
        Cmd::SetConfig { key, value } => {
            let mut cfg = cfg::Config::load();
            cfg.set(&key, &value)?;
            cfg.save().map_err(|e| format!("save config: {e}"))?;
            if cfg::applies_live(&key) {
                st.apply_live(&cfg).await;
            }
            Ok(if cfg::applies_live(&key) {
                format!("{key} = {value}")
            } else {
                format!("{key} = {value} (applies on the next start)")
            })
        }
    }
}

/// Own thread: the UI only ever reads the published `View`.
fn spawn_worker(st: Arc<State>, view: Arc<Mutex<View>>, rx: Receiver<Cmd>, log_lines: usize) {
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(e) => {
                view.lock().unwrap().note = format!("runtime: {e}");
                return;
            }
        };
        while let Ok(cmd) = rx.recv() {
            let note = rt.block_on(act(&st, cmd));
            let mut fresh = rt.block_on(snapshot(&st, log_lines));
            match note {
                Ok(msg) if !msg.is_empty() => fresh.note = msg,
                Ok(_) => {}
                Err(e) => fresh.note = e,
            }
            *view.lock().unwrap() = fresh;
        }
    });
}

struct App {
    view: Arc<Mutex<View>>,
    tx: Sender<Cmd>,
    tab: usize,
    sel: usize,
    /// Index into `FILTERS`, used by the log tab.
    filter: usize,
    /// Row picked on the settings tab.
    setting: usize,
    /// `(key, buffer)` while a setting is being edited.
    editing: Option<(String, String)>,
    quit: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // This process is the app *and* the server.
    let daemon = embed::host(cfg::Config::load())?;
    daemon.wait_ready()?;
    let startup = daemon.state.cfg.clone();
    let refresh = Duration::from_secs(if startup.ui_refresh_secs > 0 { startup.ui_refresh_secs } else { 2 });
    let log_lines = if startup.ui_log_lines > 0 { startup.ui_log_lines } else { 500 };

    let view = Arc::new(Mutex::new(View::default()));
    let (tx, rx) = channel();
    spawn_worker(daemon.state.clone(), view.clone(), rx, log_lines);
    let _ = tx.send(Cmd::Refresh);

    enable_raw_mode()?;
    let mut out = std::io::stdout();
    execute!(out, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(out))?;

    let mut app = App { view, tx, tab: 0, sel: 0, filter: 0, setting: 0, editing: None, quit: false };
    let mut last = Instant::now();
    while !app.quit {
        let snapshot = app.view.lock().expect("view lock").clone();
        terminal.draw(|f| draw(f, &app, &snapshot))?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(KeyEvent { code, kind, .. }) = event::read()? {
                if kind == KeyEventKind::Press {
                    app.on_key(code);
                }
            }
        }
        if last.elapsed() >= refresh {
            let _ = app.tx.send(Cmd::Refresh);
            last = Instant::now();
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    // Leaving the app stops the services it was hosting.
    daemon.stop();
    Ok(())
}

impl App {
    fn on_key(&mut self, code: KeyCode) {
        // While a setting is being typed, every key belongs to the buffer.
        if let Some((key, mut buf)) = self.editing.take() {
            match code {
                KeyCode::Esc => return,
                KeyCode::Enter => {
                    let _ = self.tx.send(Cmd::SetConfig { key, value: buf });
                }
                KeyCode::Backspace => {
                    buf.pop();
                    self.editing = Some((key, buf));
                }
                KeyCode::Char(c) => {
                    buf.push(c);
                    self.editing = Some((key, buf));
                }
                _ => self.editing = Some((key, buf)),
            }
            return;
        }

        let (invites, settings) = {
            let v = self.view.lock().expect("view lock");
            (v.invites.len(), v.settings.clone())
        };
        match code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Esc => self.quit = true,
            KeyCode::Char('1') => self.tab = 0,
            KeyCode::Char('2') => self.tab = 1,
            KeyCode::Char('3') => self.tab = 2,
            KeyCode::Char('4') => self.tab = 3,
            KeyCode::Char('5') => self.tab = 4,
            KeyCode::Char('6') => self.tab = 5,
            KeyCode::Tab | KeyCode::Right => self.tab = (self.tab + 1) % TABS.len(),
            KeyCode::BackTab | KeyCode::Left => self.tab = (self.tab + TABS.len() - 1) % TABS.len(),
            KeyCode::Down => match self.tab {
                2 => self.sel = (self.sel + 1).min(invites.saturating_sub(1)),
                5 => self.setting = (self.setting + 1).min(settings.len().saturating_sub(1)),
                _ => {}
            },
            KeyCode::Up => match self.tab {
                2 => self.sel = self.sel.saturating_sub(1),
                5 => self.setting = self.setting.saturating_sub(1),
                _ => {}
            },
            KeyCode::Enter if self.tab == 5 => {
                let Some((key, value)) = settings.get(self.setting).cloned() else { return };
                let is_bool = cfg::field(&key).is_some_and(|f| f.kind == cfg::Kind::Bool);
                if is_bool {
                    // a boolean has nothing to type: flip it
                    let next = if value == "true" { "false" } else { "true" };
                    let _ = self.tx.send(Cmd::SetConfig { key, value: next.to_string() });
                } else {
                    let buf = if value == "null" { String::new() } else { value };
                    self.editing = Some((key, buf));
                }
            }
            KeyCode::Char('r') => {
                let _ = self.tx.send(Cmd::Refresh);
            }
            KeyCode::Char('s') => {
                let _ = self.tx.send(Cmd::Scan);
            }
            KeyCode::Char('c') => {
                let _ = self.tx.send(Cmd::NewCode);
            }
            KeyCode::Char('f') => self.filter = (self.filter + 1) % FILTERS.len(),
            KeyCode::Char('a') => {
                if let Some(inv) = self.selected_invite() {
                    let _ = self.tx.send(Cmd::Accept(inv));
                }
            }
            KeyCode::Char('d') => {
                if let Some(inv) = self.selected_invite() {
                    let _ = self.tx.send(Cmd::Deny(inv));
                }
            }
            _ => {}
        }
    }

    fn selected_invite(&self) -> Option<String> {
        let view = self.view.lock().ok()?;
        view.invites.get(self.sel).map(|i| i.id.clone())
    }
}

fn draw(f: &mut ratatui::Frame, app: &App, view: &View) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3), Constraint::Length(2)])
        .split(f.area());

    let titles: Vec<Line> = TABS.iter().map(|t| Line::from(*t)).collect();
    f.render_widget(
        Tabs::new(titles)
            .select(app.tab)
            .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .block(Block::default().borders(Borders::ALL).title(" dtu ")),
        chunks[0],
    );

    let items: Vec<ListItem> = match app.tab {
        0 => view.status.iter().map(|l| ListItem::new(l.clone())).collect(),
        1 => view.devices.iter().map(|l| ListItem::new(l.clone())).collect(),
        2 => view
            .invites
            .iter()
            .map(|i| {
                ListItem::new(format!(
                    "{} from {} at {} (wants {})",
                    i.id, i.from, i.addr, i.expires_at
                ))
            })
            .collect(),
        3 => view.code.iter().map(|l| ListItem::new(l.clone())).collect(),
        4 => {
            let keep = FILTERS[app.filter.min(FILTERS.len() - 1)].1;
            view.logs
                .iter()
                .filter(|e| keep.is_none_or(|k| e.event == k))
                .map(|e| ListItem::new(e.line()))
                .collect()
        }
        _ => view
            .settings
            .iter()
            .map(|(key, value)| {
                let f = cfg::field(key);
                let when = match f {
                    Some(f) if f.restart => "next start",
                    _ => "live",
                };
                let shown = match &app.editing {
                    Some((editing, buf)) if editing == key => format!("{buf}█"),
                    _ => value.clone(),
                };
                ListItem::new(format!(
                    "{key:<22} {shown:<24} {when:<10} {}",
                    f.map(|f| f.help).unwrap_or("")
                ))
            })
            .collect(),
    };
    let empty = ListItem::new("nothing here yet");
    let mut state = ListState::default();
    if app.tab == 2 && !view.invites.is_empty() {
        state.select(Some(app.sel.min(view.invites.len() - 1)));
    }
    if app.tab == 5 && !view.settings.is_empty() {
        state.select(Some(app.setting.min(view.settings.len() - 1)));
    }
    let hint = match app.tab {
        2 => "up/down, a accept, d deny",
        4 => "f filter",
        5 => "up/down, enter edit/toggle",
        _ => "",
    };
    let title = if app.tab == 4 {
        format!(" log ({}) {hint} ", FILTERS[app.filter.min(FILTERS.len() - 1)].0)
    } else {
        format!(" {} {hint} ", TABS[app.tab])
    };
    f.render_stateful_widget(
        List::new(if items.is_empty() { vec![empty] } else { items })
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol("> ")
            .block(Block::default().borders(Borders::ALL).title(title)),
        chunks[1],
        &mut state,
    );

    let help = if let Some((key, buf)) = &app.editing {
        format!("{key} = {buf}█   enter save · esc cancel")
    } else if !view.note.is_empty() {
        view.note.lines().last().unwrap_or_default().to_string()
    } else {
        format!("1-{} tab  r refresh  s scan  c new code  f filter  q quit", TABS.len())
    };
    f.render_widget(
        Paragraph::new(help).block(Block::default().borders(Borders::TOP)),
        chunks[2],
    );
}
