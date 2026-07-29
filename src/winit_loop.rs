use std::time::Duration;

use smithay::{
    backend::{
        renderer::{
            element::{texture::TextureRenderElement, Element, RenderElement},
            gles::GlesTexture,
            glow::GlowRenderer,
            Frame, Renderer,
        },
        winit::{self, WinitEvent},
    },
    desktop::Window,
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::EventLoop,
        winit::{
            dpi::PhysicalSize,
            window::{WindowAttributes, WindowButtons},
        },
    },
    utils::{Point, Rectangle, Scale, Size, Transform},
};

use crate::{CalloopData, Wdroid};

pub fn init_winit(
    event_loop: &mut EventLoop<CalloopData>,
    data: &mut CalloopData,
) -> Result<(), Box<dyn std::error::Error>> {
    let display_handle = &mut data.display_handle;
    let state = &mut data.state;

    let size = state.fixed_size;

    let attributes = WindowAttributes::default()
        .with_title("wdroid")
        .with_inner_size(PhysicalSize::new(size.w as u32, size.h as u32))
        .with_min_inner_size(PhysicalSize::new(size.w as u32, size.h as u32))
        .with_max_inner_size(PhysicalSize::new(size.w as u32, size.h as u32))
        .with_resizable(false)
        .with_enabled_buttons(WindowButtons::CLOSE | WindowButtons::MINIMIZE);

    let (mut backend, winit) = winit::init_from_attributes::<GlowRenderer>(attributes)?;

    state.window_size = backend.window_size();

    // The one mode clients ever see — identical to the xdg configure size.
    // The real window may differ (WSLg ignores non-resizable hints and lets
    // the user maximize); we scale the view instead of ever resizing this.
    let mode = Mode {
        size,
        refresh: 60_000,
    };

    let output = Output::new(
        "wdroid".to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "wdroid".into(),
            model: "wdroid".into(),
        },
    );
    let _global = output.create_global::<Wdroid>(display_handle);
    output.change_current_state(Some(mode), Some(Transform::Flipped180), None, Some((0, 0).into()));
    output.set_preferred(mode);

    state.space.map_output(&output, (0, 0));

    event_loop.handle().insert_source(winit, move |event, _, data| {
        let display = &mut data.display_handle;
        let state = &mut data.state;

        match event {
            WinitEvent::Resized { size: new_size, .. } => {
                // The Android output never changes; the view scales instead.
                if new_size != state.window_size {
                    tracing::info!("window resized to {new_size:?}; scaling the fixed-size view");
                }
                state.window_size = new_size;
            }
            WinitEvent::Input(event) => state.process_input_event(event),
            WinitEvent::Redraw => {
                if let Some(after) = state.debug_maximize_after {
                    if state.start_time.elapsed() >= after {
                        state.debug_maximize_after = None;
                        tracing::warn!("debug: self-maximizing now");
                        backend.window().set_maximized(true);
                    }
                }

                let window_size = backend.window_size();
                state.window_size = window_size;
                let full = Rectangle::from_size(window_size);
                let (view_scale, view_offset) = state.view_transform();

                let has_client = state.space.elements().next().is_some();
                let status = state.session.status_text.clone();

                let egui_element: Option<TextureRenderElement<GlesTexture>> = state
                    .egui
                    .render(
                        |ctx| crate::ui::overlay(ctx, &status, has_client),
                        backend.renderer(),
                        Rectangle::from_size(window_size.to_logical(1)),
                        1.0,
                        1.0,
                    )
                    .map_err(|err| tracing::warn!("egui render failed: {err}"))
                    .ok();

                let space_elements = smithay::desktop::space::space_render_elements::<_, Window, _>(
                    backend.renderer(),
                    [&state.space],
                    &output,
                    1.0,
                )
                .unwrap_or_default();

                {
                    let (renderer, mut framebuffer) = backend.bind().unwrap();
                    let mut frame = renderer
                        .render(&mut framebuffer, window_size, Transform::Flipped180)
                        .unwrap();
                    frame.clear([0.0, 0.0, 0.0, 1.0].into(), &[full]).unwrap();

                    // The Android view area in window coordinates. Elements can
                    // overflow it (hwcomposer maps Android layers as subsurfaces,
                    // and e.g. parallax wallpapers are wider than the screen) —
                    // the small framebuffer used to clip that implicitly, so clip
                    // every draw to the view rect explicitly.
                    let view_rect = Rectangle::<i32, smithay::utils::Physical>::new(
                        Point::from((
                            view_offset.x.round() as i32,
                            view_offset.y.round() as i32,
                        )),
                        Size::from((
                            (state.fixed_size.w as f64 * view_scale).round() as i32,
                            (state.fixed_size.h as f64 * view_scale).round() as i32,
                        )),
                    );

                    // Space elements are returned front-to-back; draw back-to-front.
                    for element in space_elements.iter().rev() {
                        let geo = element.geometry(Scale::from(1.0));
                        tracing::debug!("element geometry {geo:?}");
                        let dst = Rectangle::new(
                            Point::from((
                                (geo.loc.x as f64 * view_scale + view_offset.x).round() as i32,
                                (geo.loc.y as f64 * view_scale + view_offset.y).round() as i32,
                            )),
                            Size::from((
                                (geo.size.w as f64 * view_scale).round().max(1.0) as i32,
                                (geo.size.h as f64 * view_scale).round().max(1.0) as i32,
                            )),
                        );
                        // damage is dst-local; restrict it to the visible part of
                        // the view rect so nothing paints into the letterbox bars.
                        let Some(visible) = dst.intersection(view_rect) else {
                            continue;
                        };
                        let mut local = visible;
                        local.loc -= dst.loc;
                        if let Err(err) = RenderElement::<GlowRenderer>::draw(
                            element,
                            &mut frame,
                            element.src(),
                            dst,
                            &[local],
                            &[],
                        ) {
                            tracing::warn!("element draw failed: {err}");
                        }
                    }

                    if let Some(egui_el) = egui_element.as_ref() {
                        let egui_geo = egui_el.geometry(Scale::from(1.0));
                        if let Err(err) = RenderElement::<GlowRenderer>::draw(
                            egui_el,
                            &mut frame,
                            egui_el.src(),
                            egui_geo,
                            &[Rectangle::from_size(egui_geo.size)],
                            &[],
                        ) {
                            tracing::warn!("egui draw failed: {err}");
                        }
                    }

                    if let Err(err) = frame.finish() {
                        tracing::warn!("frame finish failed: {err}");
                    }
                }
                backend.submit(Some(&[full])).unwrap();

                // hwcomposer paces its rendering on these.
                state.space.elements().for_each(|window| {
                    window.send_frame(
                        &output,
                        state.start_time.elapsed(),
                        Some(Duration::ZERO),
                        |_, _| Some(output.clone()),
                    )
                });

                state.space.refresh();
                state.popups.cleanup();
                let _ = display.flush_clients();

                backend.window().request_redraw();
            }
            WinitEvent::CloseRequested => {
                state.loop_signal.stop();
            }
            _ => (),
        };
    })?;

    Ok(())
}
