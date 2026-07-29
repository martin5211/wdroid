use std::{
    ffi::OsString,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use crate::cli::Cli;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Not started (--no-autostart), stopped by us, or gave up after retries.
    Idle,
    /// Waiting for a stale/previous session to actually stop before launching:
    /// starting while a stop is mid-flight corrupts the session state.
    StoppingStale { since: Instant },
    /// Launcher spawned, waiting for `waydroid status` to report RUNNING.
    Booting,
    /// Session RUNNING; show-full-ui has been fired once.
    Running,
    /// Session vanished externally; a restart attempt is scheduled.
    Restarting { due: Instant },
}

pub struct SessionManager {
    launcher: PathBuf,
    socket: OsString,
    pub autostart: bool,
    phase: Phase,
    children: Vec<Child>,
    last_status_poll: Instant,
    restart_attempts: u32,
    /// Human-readable state for the placeholder/overlay.
    pub status_text: String,
}

const MAX_RESTART_ATTEMPTS: u32 = 3;

impl SessionManager {
    pub fn new(cli: &Cli, socket: OsString) -> Self {
        Self {
            launcher: cli.launcher_path(),
            socket,
            autostart: !cli.no_autostart,
            phase: Phase::Idle,
            children: Vec::new(),
            last_status_poll: Instant::now() - Duration::from_secs(60),
            restart_attempts: 0,
            status_text: "session not started".into(),
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Start the session. If a stale session exists (or a previous instance's
    /// stop is still mid-flight), wait for it to be gone first — the poll()
    /// state machine drives the wait so the event loop never blocks long.
    pub fn start(&mut self) {
        if !matches!(self.phase, Phase::Idle | Phase::Restarting { .. }) {
            return;
        }
        self.preflight_prop_check();

        if session_running() {
            tracing::info!("stale waydroid session is RUNNING — stopping it first");
            self.status_text = "stopping stale session…".into();
            run_waydroid(&["session", "stop"]);
            self.phase = Phase::StoppingStale { since: Instant::now() };
        } else {
            self.spawn_launcher();
        }
    }

    fn spawn_launcher(&mut self) {
        // Fall back to plain `waydroid session start` when the launcher script
        // is absent (fresh installs, non-WSL machines, flatpak sandbox).
        let mut command = if self.launcher.exists() {
            tracing::info!(launcher = %self.launcher.display(), socket = ?self.socket, "starting waydroid session");
            Command::new(&self.launcher)
        } else {
            tracing::info!(socket = ?self.socket, "launcher script not found — running `waydroid session start` directly");
            let mut c = Command::new("waydroid");
            c.args(["session", "start"]);
            c
        };
        match command
            .env("WAYLAND_DISPLAY", &self.socket)
            .stdin(Stdio::null())
            .spawn()
        {
            Ok(child) => {
                self.children.push(child);
                self.phase = Phase::Booting;
                self.status_text = "starting Android…".into();
            }
            Err(err) => {
                tracing::error!("failed to spawn {}: {err}", self.launcher.display());
                self.status_text = format!("launcher failed: {err}");
                self.phase = Phase::Idle;
            }
        }
    }

    /// Stop the session and wait for it. Used on shutdown; safe in any phase.
    pub fn stop_blocking(&mut self) {
        self.reap();
        tracing::info!("stopping waydroid session");
        self.status_text = "session stopped".into();
        run_waydroid(&["session", "stop"]);
        self.phase = Phase::Idle;
    }

    /// Driven by the 1s timer: reap children and advance the state machine.
    pub fn poll(&mut self) {
        self.reap();

        match self.phase {
            Phase::Idle => {}
            Phase::StoppingStale { since } => {
                if !session_running() {
                    self.spawn_launcher();
                } else if since.elapsed() > Duration::from_secs(15) {
                    tracing::warn!("stale session did not stop within 15s — starting anyway");
                    self.spawn_launcher();
                }
            }
            Phase::Booting => {
                if self.poll_status_due(Duration::from_secs(2)) && session_running() {
                    tracing::info!("session RUNNING — launching full UI");
                    self.status_text = "session running".into();
                    self.phase = Phase::Running;
                    self.restart_attempts = 0;
                    match Command::new("waydroid")
                        .arg("show-full-ui")
                        .stdin(Stdio::null())
                        .spawn()
                    {
                        Ok(child) => self.children.push(child),
                        Err(err) => tracing::warn!("failed to run show-full-ui: {err}"),
                    }
                }
            }
            Phase::Running => {
                if self.poll_status_due(Duration::from_secs(5)) && !session_running() {
                    self.schedule_restart("session stopped externally");
                }
            }
            Phase::Restarting { due } => {
                if Instant::now() >= due {
                    self.start();
                }
            }
        }
    }

    /// The Android toplevel disappeared. If the session itself is gone,
    /// schedule a restart; a surface teardown while the session lives is
    /// normal (happens once during boot) and resolves on its own.
    pub fn client_gone(&mut self) {
        if self.phase == Phase::Running && !session_running() {
            self.schedule_restart("Android disconnected");
        }
    }

    fn schedule_restart(&mut self, why: &str) {
        if self.restart_attempts >= MAX_RESTART_ATTEMPTS {
            tracing::warn!("{why}; giving up after {MAX_RESTART_ATTEMPTS} restart attempts");
            self.status_text = format!("{why} — gave up; restart wdroid to retry");
            self.phase = Phase::Idle;
            return;
        }
        self.restart_attempts += 1;
        tracing::warn!(
            "{why}; restarting session (attempt {}/{MAX_RESTART_ATTEMPTS})",
            self.restart_attempts
        );
        self.status_text = format!("{why} — restarting…");
        self.phase = Phase::Restarting {
            due: Instant::now() + Duration::from_secs(3),
        };
    }

    fn poll_status_due(&mut self, every: Duration) -> bool {
        if self.last_status_poll.elapsed() >= every {
            self.last_status_poll = Instant::now();
            true
        } else {
            false
        }
    }

    fn reap(&mut self) {
        self.children.retain_mut(|child| match child.try_wait() {
            Ok(Some(_)) => false,
            Ok(None) => true,
            Err(_) => false,
        });
    }

    /// persist.waydroid.width/height in waydroid.cfg crash-loop the ATV image's
    /// hwcomposer against our fixed-size configure — warn loudly before boot.
    fn preflight_prop_check(&self) {
        if let Ok(cfg) = std::fs::read_to_string("/var/lib/waydroid/waydroid.cfg") {
            if cfg.contains("persist.waydroid.width") || cfg.contains("persist.waydroid.height") {
                tracing::warn!(
                    "waydroid.cfg sets persist.waydroid.width/height — this conflicts with \
                     wdroid's fixed-size configure and is known to SIGABRT-loop hwcomposer"
                );
            }
        }
    }
}

fn session_running() -> bool {
    Command::new("waydroid")
        .arg("status")
        .stdin(Stdio::null())
        .output()
        .ok()
        .map(|out| {
            let text = String::from_utf8_lossy(&out.stdout);
            text.lines()
                .any(|l| l.starts_with("Session:") && l.contains("RUNNING"))
        })
        .unwrap_or(false)
}

fn run_waydroid(args: &[&str]) {
    match Command::new("waydroid")
        .args(args)
        .stdin(Stdio::null())
        .status()
    {
        Ok(status) if !status.success() => {
            tracing::warn!("waydroid {args:?} exited with {status}");
        }
        Err(err) => tracing::warn!("failed to run waydroid {args:?}: {err}"),
        _ => {}
    }
}
