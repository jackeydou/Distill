//! `distill ui`: the local web UI (spec D7). One server per device, at the fixed address
//! `http://distill.localhost:<ui.port>`. It starts when the user runs `distill ui`, or in
//! the background when `distill mcp` hands an agent a link, and runs until `distill ui stop`
//! or logout. It is never registered as a system service.
//!
//! The health endpoint doubles as the single-instance lock: before binding, a process asks
//! the port whether Distill is already there.

mod api;
mod auth;
mod mcp;
mod server;

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use distill_core::Distill;
use distill_core::config::{Dirs, LocalConfig};

pub(crate) use self::mcp::{McpResponse, request as mcp_request};

use self::api::Health;
use self::auth::Auth;

/// How long `ensure_running` waits for a background server to answer.
const START_TIMEOUT: Duration = Duration::from_secs(5);

pub struct Options {
    pub port: Option<u16>,
    pub background: bool,
    pub open_browser: bool,
}

/// Runs the server in the foreground, or, when one is already running, authorizes a
/// browser for it and returns.
pub fn run(opts: Options) -> Result<()> {
    let dirs = Dirs::discover()?;
    let d = Distill::open_in(dirs.clone())?;
    let port = match (opts.port, std::env::var("PORT")) {
        (Some(port), _) => port,
        (None, Ok(env)) => env
            .parse()
            .with_context(|| format!("PORT={env} is not a port number"))?,
        (None, Err(_)) => d.config.ui.port,
    };
    let origin = origin(port);

    if probe(port).is_some() {
        if !opts.background {
            authorize_browser(&dirs, &origin, opts.open_browser)?;
        }
        return Ok(());
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let bound = match runtime.block_on(server::bind(port)) {
        Ok(bound) => bound,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            // Another `distill ui` may have won the race to bind.
            if wait_for_health(port, Duration::from_millis(500)) {
                if !opts.background {
                    authorize_browser(&dirs, &origin, opts.open_browser)?;
                }
                return Ok(());
            }
            bail!("{}", port_taken(port));
        }
        Err(e) => return Err(e).with_context(|| format!("listening on 127.0.0.1:{port}")),
    };
    if !opts.background {
        println!("Distill is running at {origin}  (Ctrl-C to stop)");
        authorize_browser(&dirs, &origin, opts.open_browser)?;
    }
    let vault = d.vault.root().to_path_buf();
    drop(d);
    runtime.block_on(server::serve(bound, dirs, &vault, port))
}

/// `distill ui stop`. Returns false when no server was running.
pub fn stop() -> Result<bool> {
    let dirs = Dirs::discover()?;
    let port = LocalConfig::load(&dirs)?.ui.port;
    if probe(port).is_none() {
        return Ok(false);
    }
    let auth = Auth::load_or_create(&dirs.data_dir.join("ui"))?;
    let res = agent()
        .post(format!("http://127.0.0.1:{port}/api/shutdown"))
        .header("Host", distill_core::config::UI_HOST)
        .header("Authorization", format!("Bearer {}", auth.secret()))
        .send_empty()
        .with_context(|| format!("asking the server on port {port} to stop"))?;
    anyhow::ensure!(
        res.status().is_success(),
        "the server on port {port} refused to stop: HTTP {}",
        res.status()
    );
    let deadline = Instant::now() + START_TIMEOUT;
    while probe(port).is_some() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(true)
}

/// Makes sure a server answers on the configured port, starting one in the background if
/// needed. Used by `distill mcp` before it returns a link. Returns the origin.
pub fn ensure_running(config: &LocalConfig, dirs: &Dirs) -> Result<String> {
    let port = config.ui.port;
    if probe(port).is_some() {
        return Ok(origin(port));
    }
    let exe = std::env::current_exe().context("finding the distill binary")?;
    let log_path = dirs.data_dir.join("ui").join("server.log");
    if let Some(dir) = log_path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("opening {}", log_path.display()))?;
    let mut cmd = Command::new(&exe);
    cmd.args(["ui", "--background"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log);
    detach(&mut cmd);
    let mut child = cmd
        .spawn()
        .with_context(|| format!("starting {} ui --background", exe.display()))?;
    // Reap the server when it exits so it does not linger as a zombie of this process.
    std::thread::spawn(move || child.wait());
    if !wait_for_health(port, START_TIMEOUT) {
        bail!(
            "the web UI did not start on port {port} within {}s; see {}",
            START_TIMEOUT.as_secs(),
            log_path.display()
        );
    }
    Ok(origin(port))
}

/// Starts the server in its own process group so it outlives the agent session or the
/// terminal that started it.
fn detach(cmd: &mut Command) {
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(cmd, 0);
    #[cfg(windows)]
    {
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        std::os::windows::process::CommandExt::creation_flags(
            cmd,
            DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP,
        );
    }
}

fn authorize_browser(dirs: &Dirs, origin: &str, open_browser: bool) -> Result<()> {
    let auth = Auth::load_or_create(&dirs.data_dir.join("ui"))?;
    let url = format!("{origin}/auth?token={}", auth.issue_token()?);
    if open_browser {
        if let Err(e) = open::that_detached(&url) {
            eprintln!("distill ui: could not open a browser ({e}); open this link instead:");
            println!("{url}");
        }
    } else {
        println!("Open this link to authorize a browser (valid for 5 minutes, once):\n{url}");
    }
    Ok(())
}

fn origin(port: u16) -> String {
    format!("http://{}:{port}", distill_core::config::UI_HOST)
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(2)))
        .http_status_as_error(false)
        .build()
        .into()
}

/// The Distill server answering on `port`, if any.
fn probe(port: u16) -> Option<Health> {
    let mut res = agent()
        .get(format!("http://127.0.0.1:{port}/api/health"))
        .header("Host", distill_core::config::UI_HOST)
        .call()
        .ok()?;
    let body = res.body_mut().read_to_string().ok()?;
    serde_json::from_str::<Health>(&body)
        .ok()
        .filter(|h| h.app == "distill")
}

fn wait_for_health(port: u16, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if probe(port).is_some() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn port_taken(port: u16) -> String {
    let who = listeners::get_process_by_port(port, listeners::Protocol::TCP)
        .map(|p| format!("{} (pid {})", p.name, p.pid))
        .unwrap_or_else(|_| "another program".into());
    format!(
        "port {port} is in use by {who}. Stop it, or move Distill to another port with \
         `distill config set ui.port <port>`; links saved with the old port stop working."
    )
}
