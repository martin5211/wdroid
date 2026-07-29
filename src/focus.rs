use smithay::{
    input::{
        pointer::{
            AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent,
            GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent, GestureSwipeEndEvent,
            GestureSwipeUpdateEvent, MotionEvent, PointerTarget, RelativeMotionEvent,
        },
        Seat,
    },
    reexports::wayland_server::{backend::ObjectId, protocol::wl_surface::WlSurface},
    utils::{IsAlive, Serial},
    wayland::seat::WaylandFocus,
};
use smithay_egui::EguiState;

use crate::Wdroid;

/// Pointer focus is either the Android surface or the overlay UI.
#[derive(Debug, Clone, PartialEq)]
pub enum PointerFocusTarget {
    WlSurface(WlSurface),
    Egui(EguiState),
}

impl IsAlive for PointerFocusTarget {
    #[inline]
    fn alive(&self) -> bool {
        match self {
            PointerFocusTarget::WlSurface(w) => w.alive(),
            PointerFocusTarget::Egui(e) => e.alive(),
        }
    }
}

impl WaylandFocus for PointerFocusTarget {
    #[inline]
    fn wl_surface(&self) -> Option<std::borrow::Cow<'_, WlSurface>> {
        match self {
            PointerFocusTarget::WlSurface(w) => Some(std::borrow::Cow::Borrowed(w)),
            PointerFocusTarget::Egui(_) => None,
        }
    }

    #[inline]
    fn same_client_as(&self, object_id: &ObjectId) -> bool {
        match self {
            PointerFocusTarget::WlSurface(w) => w.same_client_as(object_id),
            PointerFocusTarget::Egui(_) => false,
        }
    }
}

macro_rules! delegate {
    ($self:ident, $m:ident, $($arg:expr),*) => {
        match $self {
            PointerFocusTarget::WlSurface(w) => PointerTarget::$m(w, $($arg),*),
            PointerFocusTarget::Egui(e) => PointerTarget::$m(e, $($arg),*),
        }
    };
}

impl PointerTarget<Wdroid> for PointerFocusTarget {
    fn enter(&self, seat: &Seat<Wdroid>, data: &mut Wdroid, event: &MotionEvent) {
        delegate!(self, enter, seat, data, event)
    }
    fn motion(&self, seat: &Seat<Wdroid>, data: &mut Wdroid, event: &MotionEvent) {
        delegate!(self, motion, seat, data, event)
    }
    fn relative_motion(&self, seat: &Seat<Wdroid>, data: &mut Wdroid, event: &RelativeMotionEvent) {
        delegate!(self, relative_motion, seat, data, event)
    }
    fn button(&self, seat: &Seat<Wdroid>, data: &mut Wdroid, event: &ButtonEvent) {
        delegate!(self, button, seat, data, event)
    }
    fn axis(&self, seat: &Seat<Wdroid>, data: &mut Wdroid, frame: AxisFrame) {
        delegate!(self, axis, seat, data, frame)
    }
    fn frame(&self, seat: &Seat<Wdroid>, data: &mut Wdroid) {
        delegate!(self, frame, seat, data)
    }
    fn leave(&self, seat: &Seat<Wdroid>, data: &mut Wdroid, serial: Serial, time: u32) {
        delegate!(self, leave, seat, data, serial, time)
    }
    fn gesture_swipe_begin(&self, seat: &Seat<Wdroid>, data: &mut Wdroid, event: &GestureSwipeBeginEvent) {
        delegate!(self, gesture_swipe_begin, seat, data, event)
    }
    fn gesture_swipe_update(&self, seat: &Seat<Wdroid>, data: &mut Wdroid, event: &GestureSwipeUpdateEvent) {
        delegate!(self, gesture_swipe_update, seat, data, event)
    }
    fn gesture_swipe_end(&self, seat: &Seat<Wdroid>, data: &mut Wdroid, event: &GestureSwipeEndEvent) {
        delegate!(self, gesture_swipe_end, seat, data, event)
    }
    fn gesture_pinch_begin(&self, seat: &Seat<Wdroid>, data: &mut Wdroid, event: &GesturePinchBeginEvent) {
        delegate!(self, gesture_pinch_begin, seat, data, event)
    }
    fn gesture_pinch_update(&self, seat: &Seat<Wdroid>, data: &mut Wdroid, event: &GesturePinchUpdateEvent) {
        delegate!(self, gesture_pinch_update, seat, data, event)
    }
    fn gesture_pinch_end(&self, seat: &Seat<Wdroid>, data: &mut Wdroid, event: &GesturePinchEndEvent) {
        delegate!(self, gesture_pinch_end, seat, data, event)
    }
    fn gesture_hold_begin(&self, seat: &Seat<Wdroid>, data: &mut Wdroid, event: &GestureHoldBeginEvent) {
        delegate!(self, gesture_hold_begin, seat, data, event)
    }
    fn gesture_hold_end(&self, seat: &Seat<Wdroid>, data: &mut Wdroid, event: &GestureHoldEndEvent) {
        delegate!(self, gesture_hold_end, seat, data, event)
    }
}
