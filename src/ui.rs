use egui::{Align2, Area, Color32, Id, RichText};

/// Status line in the middle of the window while there is no Android surface
/// to show. Window controls come from the host decorations (WSLg/Weston);
/// decoration-close runs the same clean shutdown path.
pub fn overlay(ctx: &egui::Context, status: &str, has_client: bool) {
    if !has_client {
        Area::new(Id::new("wdroid-status"))
            .anchor(Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(RichText::new(status).size(15.0).color(Color32::from_gray(190)));
            });
    }
}
