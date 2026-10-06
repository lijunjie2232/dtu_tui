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
    /// Last action result.
    note: String,
}

enum Cmd {
    Refresh,
    Scan,
    NewCode,
    Accept(String),
    Deny(String),
}

/// Read everything the screen shows straight out of the live core state.
async fn snapshot(st: &Arc<State>) -> View {
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
    }
}

