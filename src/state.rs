use std::{ffi::OsString, sync::Arc};

use smithay::{
    desktop::{PopupManager, Space, Window, WindowSurfaceType},
    input::{keyboard::XkbConfig, Seat, SeatState},
    reexports::{
        calloop::{generic::Generic, EventLoop, Interest, LoopSignal, Mode, PostAction},
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::wl_surface::WlSurface,
            Display, DisplayHandle,
        },
    },
    utils::{Logical, Physical, Point, Size},
    wayland::{
        compositor::{CompositorClientState, CompositorState},
        output::OutputManagerState,
        selection::data_device::DataDeviceState,
        shell::xdg::XdgShellState,
        shm::ShmState,
        socket::ListeningSocketSource,
    },
};

use smithay::utils::Rectangle;
use smithay_egui::EguiState;

use crate::{cli::Cli, session::SessionManager, CalloopData};

pub struct Wdroid {
    pub start_time: std::time::Instant,
    pub socket_name: OsString,
    pub display_handle: DisplayHandle,

    pub space: Space<Window>,
    pub loop_signal: LoopSignal,
    pub fixed_size: Size<i32, Physical>,
    /// Actual host window size — may differ from fixed_size (WSLg can maximize
    /// us regardless of hints); the view is scaled, the output never changes.
    pub window_size: Size<i32, Physical>,

    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Wdroid>,
    pub data_device_state: DataDeviceState,
    pub popups: PopupManager,

    pub seat: Seat<Self>,
    pub session: SessionManager,
    pub egui: EguiState,
    pub debug_maximize_after: Option<std::time::Duration>,
}

impl Wdroid {
    pub fn new(event_loop: &mut EventLoop<CalloopData>, display: Display<Self>, cli: &Cli) -> Self {
        let start_time = std::time::Instant::now();

        let dh = display.handle();

        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let popups = PopupManager::default();

        let mut seat: Seat<Self> = seat_state.new_wl_seat(&dh, "wdroid");

        let xkb = match (&cli.xkb_layout, &cli.xkb_variant) {
            (None, None) => XkbConfig::default(),
            (layout, variant) => XkbConfig {
                layout: layout.as_deref().unwrap_or(""),
                variant: variant.as_deref().unwrap_or(""),
                ..XkbConfig::default()
            },
        };
        seat.add_keyboard(xkb, 200, 25)
            .expect("failed to initialize xkb keyboard");
        seat.add_pointer();

        let space = Space::default();

        let socket_name = Self::init_wayland_listener(display, event_loop, &cli.socket);

        let loop_signal = event_loop.get_signal();

        let session = SessionManager::new(cli, socket_name.clone());
        let egui = EguiState::new(Rectangle::from_size(cli.size.to_logical(1)));

        Self {
            start_time,
            display_handle: dh,

            space,
            loop_signal,
            socket_name,
            fixed_size: cli.size,
            window_size: cli.size,

            compositor_state,
            xdg_shell_state,
            shm_state,
            output_manager_state,
            seat_state,
            data_device_state,
            popups,
            seat,
            session,
            egui,
            debug_maximize_after: cli.debug_maximize_after.map(std::time::Duration::from_secs),
        }
    }

    /// The one size everything must agree on: xdg configure, wl_output mode, window.
    pub fn fixed_size_logical(&self) -> Size<i32, Logical> {
        self.fixed_size.to_logical(1)
    }

    /// Aspect-fit mapping of the fixed-size Android view into the actual
    /// window: (scale, centered offset in window physical px). Identity when
    /// the window has its normal size.
    pub fn view_transform(&self) -> (f64, Point<f64, Physical>) {
        let win = self.window_size.to_f64();
        let out = self.fixed_size.to_f64();
        let scale = (win.w / out.w).min(win.h / out.h);
        let offset = Point::from(((win.w - out.w * scale) / 2.0, (win.h - out.h * scale) / 2.0));
        (scale, offset)
    }

    fn init_wayland_listener(
        display: Display<Wdroid>,
        event_loop: &mut EventLoop<CalloopData>,
        socket: &str,
    ) -> OsString {
        let listening_socket = match ListeningSocketSource::with_name(socket) {
            Ok(source) => source,
            Err(err) => {
                tracing::warn!("could not bind socket {socket:?} ({err}), falling back to auto");
                ListeningSocketSource::new_auto().expect("failed to bind any wayland socket")
            }
        };

        let socket_name = listening_socket.socket_name().to_os_string();

        let loop_handle = event_loop.handle();

        loop_handle
            .insert_source(listening_socket, move |client_stream, _, state| {
                if let Err(err) = state
                    .display_handle
                    .insert_client(client_stream, Arc::new(ClientState::default()))
                {
                    tracing::warn!("failed to accept client: {err}");
                }
            })
            .expect("failed to init the wayland socket source");

        loop_handle
            .insert_source(
                Generic::new(display, Interest::READ, Mode::Level),
                |_, display, state| {
                    // Safety: we don't drop the display
                    unsafe {
                        display.get_mut().dispatch_clients(&mut state.state).unwrap();
                    }
                    Ok(PostAction::Continue)
                },
            )
            .unwrap();

        socket_name
    }

    pub fn surface_under(&self, pos: Point<f64, Logical>) -> Option<(WlSurface, Point<f64, Logical>)> {
        self.space.element_under(pos).and_then(|(window, location)| {
            window
                .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
                .map(|(s, p)| (s, (p + location).to_f64()))
        })
    }
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}
