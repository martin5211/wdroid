mod cli;
mod focus;
mod handlers;
mod input;
mod session;
mod state;
mod ui;
mod winit_loop;

use std::time::Duration;

use clap::Parser;
use smithay::reexports::{
    calloop::{
        signals::{Signal, Signals},
        timer::{TimeoutAction, Timer},
        EventLoop,
    },
    wayland_server::{Display, DisplayHandle},
};

use cli::Cli;
pub use state::Wdroid;

pub struct CalloopData {
    state: Wdroid,
    display_handle: DisplayHandle,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    if let Ok(env_filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    } else {
        tracing_subscriber::fmt().init();
    }

    let mut event_loop: EventLoop<CalloopData> = EventLoop::try_new()?;

    let display: Display<Wdroid> = Display::new()?;
    let display_handle = display.handle();
    let state = Wdroid::new(&mut event_loop, display, &cli);

    let mut data = CalloopData {
        state,
        display_handle,
    };

    winit_loop::init_winit(&mut event_loop, &mut data)?;

    // Session state machine heartbeat.
    event_loop
        .handle()
        .insert_source(Timer::from_duration(Duration::from_secs(1)), |_, _, data| {
            data.state.session.poll();
            TimeoutAction::ToDuration(Duration::from_secs(1))
        })
        .map_err(|e| format!("failed to set up timer: {e}"))?;

    // Clean shutdown on Ctrl+C / kill.
    event_loop
        .handle()
        .insert_source(Signals::new(&[Signal::SIGINT, Signal::SIGTERM])?, |_, _, data| {
            tracing::info!("signal received — shutting down");
            data.state.loop_signal.stop();
        })?;

    tracing::info!(
        "wdroid listening on {:?} ({}x{})",
        data.state.socket_name,
        cli.size.w,
        cli.size.h
    );

    if data.state.session.autostart {
        data.state.session.start();
    } else {
        tracing::info!(
            "autostart disabled — connect clients with WAYLAND_DISPLAY={:?}",
            data.state.socket_name
        );
    }

    event_loop.run(None, &mut data, move |data| {
        let _ = data.display_handle.flush_clients();
    })?;

    // All exit paths (close button, decoration close, signals) end here.
    data.state.session.stop_blocking();

    Ok(())
}
