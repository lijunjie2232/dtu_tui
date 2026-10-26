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

const TABS: [&str; 4] = ["status", "devices", "invites", "code"];
const REFRESH: Duration = Duration::from_secs(2);

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
            let mut fresh = rt.block_on(snapshot(&st));
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
    quit: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // This process is the app *and* the server.
    let daemon = embed::host(cfg::Config::load())?;
    daemon.wait_ready()?;

    let view = Arc::new(Mutex::new(View::default()));
    let (tx, rx) = channel();
    spawn_worker(daemon.state.clone(), view.clone(), rx);
    let _ = tx.send(Cmd::Refresh);

    enable_raw_mode()?;
    let mut out = std::io::stdout();
    execute!(out, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(out))?;

    let mut app = App { view, tx, tab: 0, sel: 0, quit: false };
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
        if last.elapsed() >= REFRESH {
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
        let invites = self.view.lock().expect("view lock").invites.len();
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            KeyCode::Char('1') => self.tab = 0,
            KeyCode::Char('2') => self.tab = 1,
            KeyCode::Char('3') => self.tab = 2,
            KeyCode::Char('4') => self.tab = 3,
            KeyCode::Tab | KeyCode::Right => self.tab = (self.tab + 1) % TABS.len(),
            KeyCode::BackTab | KeyCode::Left => self.tab = (self.tab + TABS.len() - 1) % TABS.len(),
            KeyCode::Down => self.sel = (self.sel + 1).min(invites.saturating_sub(1)),
            KeyCode::Up => self.sel = self.sel.saturating_sub(1),
            KeyCode::Char('r') => {
                let _ = self.tx.send(Cmd::Refresh);
            }
            KeyCode::Char('s') => {
                let _ = self.tx.send(Cmd::Scan);
            }
            KeyCode::Char('c') => {
                let _ = self.tx.send(Cmd::NewCode);
            }
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
        _ => view.code.iter().map(|l| ListItem::new(l.clone())).collect(),
    };
    let empty = ListItem::new("nothing here yet");
    let mut state = ListState::default();
    if app.tab == 2 && !view.invites.is_empty() {
        state.select(Some(app.sel.min(view.invites.len() - 1)));
    }
    let hint = if app.tab == 2 { "up/down, a accept, d deny" } else { "" };
    f.render_stateful_widget(
        List::new(if items.is_empty() { vec![empty] } else { items })
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol("> ")
            .block(Block::default().borders(Borders::ALL).title(format!(" {} {} ", TABS[app.tab], hint))),
        chunks[1],
        &mut state,
    );

    let help = if view.note.is_empty() {
        "1-4 tab  r refresh  s scan  c new code  q quit".to_string()
    } else {
        view.note.lines().last().unwrap_or_default().to_string()
    };
    f.render_widget(
        Paragraph::new(help).block(Block::default().borders(Borders::TOP)),
        chunks[2],
    );
}
