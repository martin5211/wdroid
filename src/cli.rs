use clap::Parser;
use smithay::utils::{Physical, Size};

/// Waydroid in a window — a minimal nested Wayland compositor.
#[derive(Parser, Debug, Clone)]
#[command(name = "wdroid", version)]
pub struct Cli {
    /// Fixed window/output size as WxH. Never changes at runtime: the Waydroid
    /// ATV image crash-loops if surface geometry disagrees with the configure.
    #[arg(long, default_value = "490x896", value_parser = parse_size)]
    pub size: Size<i32, Physical>,

    /// Name of the Wayland socket to create in $XDG_RUNTIME_DIR
    #[arg(long, default_value = "wayland-wdroid")]
    pub socket: String,

    /// Do not start the Waydroid session automatically
    #[arg(long)]
    pub no_autostart: bool,

    /// Session launcher script (inherits our environment with WAYLAND_DISPLAY overridden)
    #[arg(long, default_value = "~/.local/bin/waydroid-up")]
    pub launcher: String,

    /// XKB layout override (defaults to XKB_DEFAULT_LAYOUT / system default)
    #[arg(long)]
    pub xkb_layout: Option<String>,

    /// XKB variant override
    #[arg(long)]
    pub xkb_variant: Option<String>,

    /// Debug helper: self-maximize after N seconds to exercise the snap-back path
    #[arg(long, hide = true)]
    pub debug_maximize_after: Option<u64>,
}

fn parse_size(s: &str) -> Result<Size<i32, Physical>, String> {
    let (w, h) = s
        .split_once(['x', 'X'])
        .ok_or_else(|| format!("expected WxH, got {s:?}"))?;
    let w: i32 = w.trim().parse().map_err(|e| format!("bad width: {e}"))?;
    let h: i32 = h.trim().parse().map_err(|e| format!("bad height: {e}"))?;
    if w < 100 || h < 100 || w > 8192 || h > 8192 {
        return Err(format!("unreasonable size {w}x{h}"));
    }
    Ok(Size::from((w, h)))
}

impl Cli {
    pub fn launcher_path(&self) -> std::path::PathBuf {
        if let Some(rest) = self.launcher.strip_prefix("~/") {
            if let Some(home) = std::env::var_os("HOME") {
                return std::path::PathBuf::from(home).join(rest);
            }
        }
        std::path::PathBuf::from(&self.launcher)
    }
}
