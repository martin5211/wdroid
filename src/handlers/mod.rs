mod compositor;
mod xdg_shell;

use crate::Wdroid;

use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::Resource;
use smithay::wayland::output::OutputHandler;
use smithay::wayland::selection::data_device::{
    set_data_device_focus, ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
};
use smithay::wayland::selection::SelectionHandler;
use smithay::{delegate_data_device, delegate_output, delegate_seat};

impl SeatHandler for Wdroid {
    type KeyboardFocus = WlSurface;
    type PointerFocus = crate::focus::PointerFocusTarget;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Wdroid> {
        &mut self.seat_state
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, _image: smithay::input::pointer::CursorImageStatus) {
        // Host cursor (WSLg/Weston) stays visible; nothing to draw ourselves.
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let dh = &self.display_handle;
        let client = focused.and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, client);
    }
}

delegate_seat!(Wdroid);

impl SelectionHandler for Wdroid {
    type SelectionUserData = ();
}

impl DataDeviceHandler for Wdroid {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for Wdroid {}
impl ServerDndGrabHandler for Wdroid {}

delegate_data_device!(Wdroid);

impl OutputHandler for Wdroid {}
delegate_output!(Wdroid);
