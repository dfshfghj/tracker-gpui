use gpui_component::scroll::ScrollableElement;
use gpui_component::{TitleBar, TITLE_BAR_HEIGHT};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::i18n::tr;
use crate::*;

impl Render for Probe {
    #[allow(refining_impl_trait)]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.dialog_only {
            return self.render_tracker_dialog(window, cx);
        }
        if !self.focused_once {
            self.focused_once = true;
            window.focus(&self.focus, cx);
        }
        if self.main_hwnd.is_none() {
            self.main_hwnd = native_hwnd(window);
        }
        if self.pending_tracker_window {
            self.pending_tracker_window = false;
            self.open_tracker_window(cx);
        }
        let retired = {
            let mut st = self.state.borrow_mut();
            st.tick();
            st.poll_tracking();
            st.ensure_frame_image();
            st.count_frame();
            let waiting_frame =
                st.video.is_some() && st.frame_image.as_ref().map(|(i, _)| *i) != Some(st.frame);
            if st.playing
                || waiting_frame
                || st.loading.is_some()
                || st.tracking.is_some()
                || st.tracker_open
            {
                window.request_animation_frame();
            }
            let mut drops = st.take_drops();
            drops.extend(std::mem::take(&mut st.textures_to_drop));
            drops
        };
        for image in retired {
            let _ = window.drop_image(image);
        }

        let state = self.state.clone();
        let focus = self.focus.clone();

        // ---- stage element ----
        let prepaint_state = state.clone();
        let paint_state = state.clone();
        let stage = div()
            .id("video-stage")
            .flex_1()
            .min_h(px(0.))
            .min_w(px(0.))
            .relative()
            .overflow_hidden()
            .when(state.borrow().shift_down, |this| this.cursor_crosshair())
            .track_focus(&focus)
            .on_modifiers_changed(
                cx.listener(|this, ev: &ModifiersChangedEvent, _window, cx| {
                    let shift = ev.modifiers.shift;
                    let mut st = this.state.borrow_mut();
                    if st.shift_down != shift {
                        st.shift_down = shift;
                        drop(st);
                        cx.notify();
                    }
                }),
            )
            .on_key_up(cx.listener(|this, ev: &KeyUpEvent, _window, cx| {
                if ev.keystroke.key == "shift" {
                    let mut st = this.state.borrow_mut();
                    if st.shift_down {
                        st.shift_down = false;
                        drop(st);
                        cx.notify();
                    }
                }
            }))
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _window, cx| {
                let mut st = this.state.borrow_mut();
                if st.inline_edit.is_some() {
                    match ev.keystroke.key.as_str() {
                        "escape" => st.inline_edit = None,
                        "enter" => {
                            st.apply_inline_edit();
                        }
                        "backspace" => {
                            if let Some(edit) = &mut st.inline_edit {
                                edit.text.pop();
                            }
                        }
                        key => {
                            let mut chars = key.chars();
                            if chars.clone().count() == 1
                                && !ev.keystroke.modifiers.control
                                && !ev.keystroke.modifiers.platform
                            {
                                let ch = chars.next().unwrap();
                                if ch.is_ascii_digit() || ch == '.' || ch == '-' {
                                    if let Some(edit) = &mut st.inline_edit {
                                        edit.text.push(ch);
                                        edit.error = None;
                                    }
                                }
                            }
                        }
                    }
                    drop(st);
                    cx.notify();
                    return;
                }
                if matches!(st.modal, Some(Modal::Calibration)) {
                    match ev.keystroke.key.as_str() {
                        "escape" => {
                            st.modal = None;
                            st.cal_error = None;
                        }
                        "enter" => {
                            st.apply_calibration_dialog();
                        }
                        "tab" => st.cycle_calibration_field(),
                        "backspace" => {
                            let input = match st.cal_field {
                                CalibrationField::Length => &mut st.cal_length_input,
                                CalibrationField::Unit => &mut st.cal_unit_input,
                                CalibrationField::Decimals => &mut st.cal_decimals_input,
                            };
                            input.pop();
                        }
                        key => {
                            let mut chars = key.chars();
                            if chars.clone().count() == 1
                                && !ev.keystroke.modifiers.control
                                && !ev.keystroke.modifiers.platform
                            {
                                let ch = chars.next().unwrap();
                                let allowed = match st.cal_field {
                                    CalibrationField::Length => {
                                        ch.is_ascii_digit() || ch == '.' || ch == '-'
                                    }
                                    CalibrationField::Unit => ch.is_ascii_alphabetic() || ch == '/',
                                    CalibrationField::Decimals => ch.is_ascii_digit(),
                                };
                                if allowed {
                                    let input = match st.cal_field {
                                        CalibrationField::Length => &mut st.cal_length_input,
                                        CalibrationField::Unit => &mut st.cal_unit_input,
                                        CalibrationField::Decimals => &mut st.cal_decimals_input,
                                    };
                                    input.push(ch);
                                }
                            }
                        }
                    }
                    drop(st);
                    cx.notify();
                    return;
                }
                if matches!(st.modal, Some(Modal::Operations)) {
                    if matches!(ev.keystroke.key.as_str(), "escape" | "enter") {
                        st.modal = None;
                    }
                    drop(st);
                    cx.notify();
                    return;
                }
                match ev.keystroke.key.as_str() {
                    "shift" => st.shift_down = true,
                    "space" => {
                        if st.playing {
                            st.pause();
                        } else {
                            st.play();
                        }
                    }
                    "left" => {
                        st.pause();
                        let step = if ev.keystroke.modifiers.shift { 10 } else { 1 };
                        let next = st.frame.saturating_sub(step);
                        st.set_frame(next);
                    }
                    "right" => {
                        st.pause();
                        let step = if ev.keystroke.modifiers.shift { 10 } else { 1 };
                        let next = st.frame + step;
                        st.set_frame(next);
                    }
                    "z" if ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform => {
                        st.pause();
                        if ev.keystroke.modifiers.shift {
                            st.redo_edit();
                        } else {
                            st.undo_edit();
                        }
                    }
                    "y" if ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform => {
                        st.pause();
                        st.redo_edit();
                    }
                    "c" if ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform => {
                        let text = st.table_tsv();
                        if !text.is_empty() {
                            cx.write_to_clipboard(ClipboardItem::new_string(text));
                            st.status = Some(tr("status.table_copied"));
                        }
                    }
                    "f" => st.fit(),
                    "t" => st.select_tool(Tool::Move),
                    "escape" => {
                        if st.tool_menu.is_some() {
                            st.tool_menu = None;
                        } else if st.plot_var_menu.is_some() {
                            st.plot_var_menu = None;
                        } else if st.menu_open.is_some() {
                            st.menu_open = None;
                        } else {
                            st.select_tool(Tool::Move);
                        }
                    }
                    "a" => {
                        let active = st.active;
                        if let Some(track) = st.tracks.get_mut(active) {
                            track.auto_advance = !track.auto_advance;
                            st.commit_edit();
                        }
                    }
                    "delete" | "backspace" => {
                        if st.tracking.is_some() {
                            st.stop_tracking();
                        }
                        if matches!(st.selection, Selection::Stick(_) | Selection::Track(_)) {
                            st.delete_selected_object();
                        } else if st.selection != Selection::Axes {
                            let frame = st.frame;
                            let active = st.active;
                            st.apply_edit_command(EditCommand::DeletePoint {
                                track: active,
                                frame,
                            });
                        }
                    }
                    _ => {}
                }
                drop(st);
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus, cx);
                    let mut st = this.state.borrow_mut();
                    st.inline_edit = None;
                    // The click that activates a freshly opened window must not drop a point.
                    if st.created.elapsed().as_millis() < 500 {
                        return;
                    }
                    if st.video.is_none() {
                        return;
                    }
                    if st.tracking.is_some() {
                        st.stop_tracking();
                    }
                    let sx = f32::from(ev.position.x) - st.sx;
                    let sy = f32::from(ev.position.y) - st.sy;
                    st.drag = None;
                    let (ix, iy) = st.to_img(sx, sy);
                    let frame = st.frame;
                    if st.rigid_roi_mode || st.motion_roi_mode {
                        let motion = st.motion_roi_mode;
                        st.rigid_roi_mode = false;
                        st.motion_roi_mode = false;
                        st.drag = Some(Drag::RigidRoi {
                            start: (ix, iy),
                            current: (ix, iy),
                            frame,
                            motion,
                        });
                        drop(st);
                        cx.notify();
                        return;
                    }
                    if ev.modifiers.shift {
                        let track = st.ensure_point_mass_track();
                        let track_id = st.tracks[track].id;
                        if st.apply_edit_command_transient(EditCommand::SetPoint {
                            track,
                            frame,
                            x: ix,
                            y: iy,
                        }) {
                            st.active = track;
                            st.selection = Selection::Track(track_id);
                            st.sync_plot_for_active();
                            st.drag = Some(Drag::Handle {
                                track,
                                frame,
                                index: 0,
                                created: true,
                            });
                        }
                    } else if let Some(hit) = st.hit_test(sx, sy) {
                        match &hit {
                            Drag::AxisOrigin | Drag::AxisAngle | Drag::AxisLine { .. } => {
                                st.selection = Selection::Axes;
                            }
                            Drag::CalStick { stick, .. } | Drag::StickMove { stick, .. } => {
                                st.active_stick = *stick;
                                st.selection = Selection::Stick(*stick);
                            }
                            Drag::Handle { track, .. }
                            | Drag::TrackMove { track, .. }
                            | Drag::RigidRotate { track, .. } => {
                                st.active = *track;
                                st.selection = Selection::Track(st.tracks[*track].id);
                                st.sync_plot_for_active();
                            }
                            _ => {}
                        }
                        st.drag = Some(hit);
                    } else {
                        st.selection = Selection::None;
                        match st.tool {
                            Tool::Move => {}
                            Tool::Origin => {
                                st.apply_edit_command(EditCommand::SetOrigin {
                                    frame,
                                    x: ix,
                                    y: iy,
                                });
                                st.selection = Selection::Axes;
                            }
                            Tool::OriginOffset => {
                                st.coords.set_origin(frame, [ix as f64, iy as f64]);
                                st.selection = Selection::Axes;
                                st.drag = Some(Drag::Origin { frame });
                            }
                            Tool::Tape | Tool::Protractor | Tool::Circle => {
                                let kind = match st.tool {
                                    Tool::Tape => MeasurementKind::Tape,
                                    Tool::Protractor => MeasurementKind::Protractor,
                                    _ => MeasurementKind::Circle,
                                };
                                let track = st.ensure_measurement_track(kind);
                                if !st.tracks[track].locked
                                    && st.tracks[track].measurement_step(frame).is_none()
                                {
                                    let step = if kind == MeasurementKind::Tape {
                                        MeasurementStep {
                                            handles: vec![(ix, iy), (ix, iy)],
                                        }
                                    } else {
                                        track::MeasurementTrack::default_step(kind, ix, iy)
                                    };
                                    st.tracks[track].set_measurement_step(frame, step);
                                    st.active = track;
                                    st.selection = Selection::Track(st.tracks[track].id);
                                    st.sync_plot_for_active();
                                    if kind == MeasurementKind::Tape {
                                        st.drag = Some(Drag::Handle {
                                            track,
                                            frame,
                                            index: 1,
                                            created: true,
                                        });
                                    } else {
                                        st.commit_edit();
                                    }
                                }
                            }
                        }
                    }
                    drop(st);
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _window, cx| {
                let mut st = this.state.borrow_mut();
                st.shift_down = ev.modifiers.shift;
                let sx = f32::from(ev.position.x) - st.sx;
                let sy = f32::from(ev.position.y) - st.sy;
                let dragged = st.drag.clone();
                match dragged {
                    Some(Drag::Handle {
                        track,
                        frame,
                        index,
                        ..
                    }) => {
                        let (ix, iy) = st.to_img(sx, sy);
                        if let Some(track) = st.tracks.get_mut(track) {
                            if !track.locked {
                                track.set_handle(frame, index, ix, iy);
                            }
                        }
                    }
                    Some(Drag::StickMove { stick, start, a, b }) => {
                        let (ix, iy) = st.to_img(sx, sy);
                        let dx = (ix - start.0) as f64;
                        let dy = (iy - start.1) as f64;
                        let frame = st.frame;
                        st.calibration.set_stick_point(
                            frame,
                            stick,
                            0,
                            (a[0] + dx) as f32,
                            (a[1] + dy) as f32,
                        );
                        st.calibration.set_stick_point(
                            frame,
                            stick,
                            1,
                            (b[0] + dx) as f32,
                            (b[1] + dy) as f32,
                        );
                    }
                    Some(Drag::TrackMove {
                        track,
                        start,
                        handles,
                    }) => {
                        let (ix, iy) = st.to_img(sx, sy);
                        let dx = ix - start.0;
                        let dy = iy - start.1;
                        let frame = st.frame;
                        if let Some(track) = st.tracks.get_mut(track) {
                            if !track.locked {
                                for (index, (x, y)) in handles.iter().enumerate() {
                                    track.set_handle(frame, index, x + dx, y + dy);
                                }
                            }
                        }
                    }
                    Some(Drag::RigidRotate {
                        track,
                        frame,
                        center,
                    }) => {
                        let (ix, iy) = st.to_img(sx, sy);
                        let dx = ix - center.0;
                        let dy = iy - center.1;
                        if dx.abs() + dy.abs() > 0.5 {
                            let angle = dy.atan2(dx);
                            if let Some(track) = st.tracks.get_mut(track) {
                                if !track.locked {
                                    track.set_rigid_angle(frame, angle);
                                }
                            }
                        }
                    }
                    Some(Drag::Origin { frame }) => {
                        let (ix, iy) = st.to_img(sx, sy);
                        st.coords.set_origin(frame, [ix as f64, iy as f64]);
                    }
                    Some(Drag::CalStick { stick, end }) => {
                        let (ix, iy) = st.to_img(sx, sy);
                        let frame = st.frame;
                        st.calibration.set_stick_point(frame, stick, end, ix, iy);
                        if let Some((a, b, length)) = st.calibration.stick(frame, stick) {
                            let dx = b[0] - a[0];
                            let dy = b[1] - a[1];
                            let pixel_length = (dx * dx + dy * dy).sqrt();
                            if pixel_length > 1e-9 && length > 1e-9 {
                                st.coords.fixed_scale = st.calibration.fixed_coordinates;
                                st.coords.set_scale_forced(frame, pixel_length / length);
                                let coords = st.coords.clone();
                                st.calibration.sync_sticks(frame, &coords, Some(stick));
                            }
                        }
                    }
                    Some(Drag::AxisOrigin) => {
                        let (ix, iy) = st.to_img(sx, sy);
                        let frame = st.frame;
                        st.coords.set_origin(frame, [ix as f64, iy as f64]);
                    }
                    Some(Drag::AxisAngle) => {
                        let (ix, iy) = st.to_img(sx, sy);
                        let frame = st.frame;
                        let origin = st.coords.origin(frame);
                        let image_dx = ix - origin[0] as f32;
                        let image_dy = iy - origin[1] as f32;
                        if image_dx.abs() > 1e-6 || image_dy.abs() > 1e-6 {
                            let angle = -(image_dy.atan2(image_dx)) as f64;
                            st.coords.set_angle(frame, angle);
                        }
                    }
                    Some(Drag::AxisLine { start, origin }) => {
                        let (ix, iy) = st.to_img(sx, sy);
                        let frame = st.frame;
                        st.coords.set_origin(
                            frame,
                            [
                                origin[0] + (ix - start.0) as f64,
                                origin[1] + (iy - start.1) as f64,
                            ],
                        );
                    }
                    Some(Drag::RigidRoi {
                        start,
                        frame,
                        motion,
                        ..
                    }) => {
                        let current = st.to_img(sx, sy);
                        st.drag = Some(Drag::RigidRoi {
                            start,
                            current,
                            frame,
                            motion,
                        });
                    }
                    None => {}
                }
                drop(st);
                cx.notify();
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _ev: &MouseUpEvent, _window, cx| {
                    let mut st = this.state.borrow_mut();
                    let dragged = st.drag.clone();
                    #[cfg(feature = "vision")]
                    if let Some(Drag::RigidRoi {
                        start,
                        current,
                        frame,
                        motion,
                    }) = dragged.clone()
                    {
                        st.drag = None;
                        st.rigid_roi_mode = false;
                        st.motion_roi_mode = false;
                        let img_w = st.img_w;
                        let img_h = st.img_h;
                        let x0 = start.0.min(current.0).clamp(0.0, img_w);
                        let y0 = start.1.min(current.1).clamp(0.0, img_h);
                        let x1 = start.0.max(current.0).clamp(0.0, img_w);
                        let y1 = start.1.max(current.1).clamp(0.0, img_h);
                        let roi = (x0, y0, x1 - x0, y1 - y0);
                        drop(st);
                        if roi.2 >= 16.0 && roi.3 >= 16.0 {
                            if motion {
                                this.start_motion_contour_tracking(roi, frame, cx);
                            } else {
                                this.start_orb_tracking(roi, frame, cx);
                            }
                        } else {
                            this.state.borrow_mut().status =
                                Some(tr("status.roi_too_small"));
                            cx.notify();
                        }
                        return;
                    }
                    let edited = matches!(
                        dragged,
                        Some(Drag::Handle { .. })
                            | Some(Drag::Origin { .. })
                            | Some(Drag::CalStick { .. })
                            | Some(Drag::StickMove { .. })
                            | Some(Drag::TrackMove { .. })
                            | Some(Drag::RigidRotate { .. })
                            | Some(Drag::AxisOrigin)
                            | Some(Drag::AxisAngle)
                            | Some(Drag::AxisLine { .. })
                    );
                    if let Some(Drag::Handle {
                        track,
                        frame,
                        created: true,
                        ..
                    }) = dragged.clone()
                    {
                        if st
                            .tracks
                            .get(track)
                            .map(|track| track.measurement_kind() == Some(MeasurementKind::Tape))
                            .unwrap_or(false)
                        {
                            let handles = st.tracks[track].handles(frame);
                            if handles.len() >= 2 {
                                let (a, b) = (handles[0], handles[1]);
                                if (a.0 - b.0).abs() < 2.0 && (a.1 - b.1).abs() < 2.0 {
                                    st.tracks[track].set_handle(frame, 1, a.0 + 120.0, a.1);
                                }
                            }
                        }
                    }
                    st.drag = None;
                    if edited {
                        st.commit_edit();
                        if st.tracker_open {
                            st.sync_tracker_overlay();
                        }
                        let point_created =
                            matches!(dragged, Some(Drag::Handle { created: true, .. }));
                        if point_created {
                            let track_index = match dragged {
                                Some(Drag::Handle { track, .. }) => track,
                                _ => st.active,
                            };
                            let (auto_advance, step) = st
                                .tracks
                                .get(track_index)
                                .filter(|track| track.is_point_mass())
                                .map(|track| (track.auto_advance, track.step_size.max(1)))
                                .unwrap_or((false, 1));
                            if auto_advance {
                                let next = (st.frame + step).min(st.frame_count.saturating_sub(1));
                                st.pause();
                                st.set_frame(next);
                            }
                        }
                    }
                    drop(st);
                    cx.notify();
                }),
            )
            .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _window, cx| {
                let mut st = this.state.borrow_mut();
                if st.video.is_none() {
                    return;
                }
                let (dx, dy) = match ev.delta {
                    ScrollDelta::Pixels(p) => (f32::from(p.x), f32::from(p.y)),
                    ScrollDelta::Lines(p) => (p.x * 20.0, p.y * 20.0),
                };
                if ev.modifiers.control || ev.modifiers.alt {
                    st.tx -= dx;
                    st.ty -= dy;
                    st.user_adjusted = true;
                    drop(st);
                    cx.notify();
                    return;
                }
                let sx = f32::from(ev.position.x) - st.sx;
                let sy = f32::from(ev.position.y) - st.sy;
                let factor = (dy * 0.0015).exp();
                let new_scale = (st.scale * factor).clamp(0.05, 40.0);
                let k = new_scale / st.scale;
                st.tx = sx - (sx - st.tx) * k;
                st.ty = sy - (sy - st.ty) * k;
                st.scale = new_scale;
                st.user_adjusted = true;
                drop(st);
                cx.notify();
            }))
            .child(
                canvas(
                    move |bounds, window, _cx| {
                        let mut st = prepaint_state.borrow_mut();
                        let ox = f32::from(bounds.origin.x);
                        let oy = f32::from(bounds.origin.y);
                        let w = f32::from(bounds.size.width);
                        let h = f32::from(bounds.size.height);
                        st.sx = ox;
                        st.sy = oy;
                        if (w - st.sw).abs() > 0.5 || (h - st.sh).abs() > 0.5 {
                            st.sw = w;
                            st.sh = h;
                            if !st.user_adjusted {
                                st.fit();
                            }
                            // First real layout: ask for one more frame so the
                            // freshly fitted scene is actually painted.
                            window.request_animation_frame();
                        }
                    },
                    move |_bounds, _pre, window, cx| {
                        let t0 = Instant::now();
                        let points = {
                            let st = paint_state.borrow();
                            draw_scene(&st, window, cx)
                        };
                        let ms = t0.elapsed().as_secs_f32() * 1000.0;
                        let mut st = paint_state.borrow_mut();
                        st.draw_ms = ms;
                        st.drawn_points = points;
                    },
                )
                .size_full(),
            )
            .child(
                div()
                    .absolute()
                    .top(px(8.))
                    .right(px(12.))
                    .text_xs()
                    .text_color(c(0xc8d7f0))
                    .child({
                        let st = state.borrow();
                        format!(
                            "frame {} / {}   {:.0}%",
                            st.frame,
                            st.frame_count.saturating_sub(1),
                            st.scale * 100.0
                        )
                    }),
            )
            .child({
                let st = state.borrow();
                if let Some(loading) = &st.loading {
                    div()
                        .absolute()
                        .top(px(0.))
                        .left(px(0.))
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(c(0x070b12).alpha(0.82))
                        .text_sm()
                        .text_color(c(0xb9cbe4))
                        .child(
                            t!(
                                "status.indexing",
                                name = loading.name,
                                pct = loading.progress.load(Ordering::Relaxed)
                            )
                            .to_string(),
                        )
                } else if let Some(error) = &st.error {
                    div()
                        .absolute()
                        .top(px(0.))
                        .left(px(0.))
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(c(0x070b12).alpha(0.82))
                        .text_sm()
                        .text_color(c(0xff6b6b))
                        .child(error.clone())
                } else {
                    div()
                }
            });

        // ---- video component: toolbar + canvas + transport + progress ----
        let toolbar = {
            let (
                tool,
                measurement_tool,
                auto_advance,
                tracking,
                has_track,
                axes_visible,
                selection,
                has_video,
                auto_detect_running,
                rigid_roi_mode,
                motion_roi_mode,
                point_detect_method,
                rigid_detect_method,
            ) = {
                let st = state.borrow();
                (
                    st.tool,
                    st.measurement_tool,
                    st.active_track()
                        .map(|track| track.auto_advance)
                        .unwrap_or(false),
                    st.tracking.is_some(),
                    st.active_track()
                        .map(|track| track.is_point_mass())
                        .unwrap_or(false),
                    st.axes_visible,
                    st.selection,
                    st.video.is_some(),
                    st.auto_detect_running,
                    st.rigid_roi_mode,
                    st.motion_roi_mode,
                    st.point_detect_method,
                    st.rigid_detect_method,
                )
            };
            #[cfg(not(feature = "vision"))]
            let _ = (
                has_video,
                auto_detect_running,
                rigid_roi_mode,
                motion_roi_mode,
                point_detect_method,
                rigid_detect_method,
            );
            let autostep_tip = if auto_advance {
                tr("toolbar.autostep_on")
            } else {
                tr("toolbar.autostep_off")
            };
            #[cfg(feature = "vision")]
            let point_group = {
                let active = match point_detect_method {
                    PointDetectMethod::AutoTracker => tracking,
                    PointDetectMethod::AutoDetect => auto_detect_running,
                };
                group_icon_button(
                    "tool-point-detect",
                    point_detect_method.icon(),
                    point_detect_method.tooltip(),
                    ToolGroup::PointDetect,
                    active,
                    has_video && !auto_detect_running,
                    cx,
                    move |this, cx| match point_detect_method {
                        PointDetectMethod::AutoTracker => {
                            let mut st = this.state.borrow_mut();
                            if !st
                                .active_track()
                                .map(|track| track.is_point_mass())
                                .unwrap_or(false)
                            {
                                st.status = Some(tr("status.need_point_track"));
                                return;
                            }
                            drop(st);
                            this.open_tracker_window(cx);
                        }
                        PointDetectMethod::AutoDetect => {
                            let mut st = this.state.borrow_mut();
                            if st.tracking.is_some() {
                                st.status = Some(tr("status.stop_tracking_first"));
                                return;
                            }
                            st.rigid_roi_mode = false;
                            st.motion_roi_mode = false;
                            drop(st);
                            this.start_motion_detection(cx);
                        }
                    },
                )
            };
            #[cfg(not(feature = "vision"))]
            let point_group = icon_button(
                "tool-autotrack",
                ICON_TARGET,
                "AutoTracker…",
                32.0,
                18.0,
                tracking,
                has_track,
                cx,
                move |this, cx| {
                    this.open_tracker_window(cx);
                },
            );
            #[cfg(feature = "vision")]
            let rigid_group = {
                let active = match rigid_detect_method {
                    RigidDetectMethod::Aruco => auto_detect_running,
                    RigidDetectMethod::Markerless => rigid_roi_mode,
                    RigidDetectMethod::MotionContour => motion_roi_mode,
                };
                group_icon_button(
                    "tool-rigid-detect",
                    rigid_detect_method.icon(),
                    rigid_detect_method.tooltip(),
                    ToolGroup::RigidDetect,
                    active,
                    has_video && !tracking && !auto_detect_running,
                    cx,
                    move |this, cx| match rigid_detect_method {
                        RigidDetectMethod::Aruco => {
                            {
                                let mut st = this.state.borrow_mut();
                                st.rigid_roi_mode = false;
                                st.motion_roi_mode = false;
                            }
                            this.start_rigid_tracking(cx);
                        }
                        RigidDetectMethod::Markerless => {
                            let mut st = this.state.borrow_mut();
                            st.rigid_roi_mode = !st.rigid_roi_mode;
                            st.motion_roi_mode = false;
                            st.tool = Tool::Move;
                            st.status = st
                                .rigid_roi_mode
                                .then(|| tr("menu.item.markerless_hint"));
                        }
                        RigidDetectMethod::MotionContour => {
                            let mut st = this.state.borrow_mut();
                            st.motion_roi_mode = !st.motion_roi_mode;
                            st.rigid_roi_mode = false;
                            st.tool = Tool::Move;
                            st.status = st
                                .motion_roi_mode
                                .then(|| tr("menu.item.motion_hint"));
                        }
                    },
                )
            };
            #[cfg(not(feature = "vision"))]
            let rigid_group = div().w(px(0.)).h(px(0.));
            div()
                .w(px(42.))
                .flex_none()
                .h_full()
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .py_2()
                .bg(c(0x0a1018))
                .border_r_1()
                .border_color(c(0x1d2a3d))
                .child(icon_button(
                    "tool-move",
                    ICON_MOVE,
                    tr("toolbar.move"),
                    32.0,
                    18.0,
                    tool == Tool::Move,
                    true,
                    cx,
                    |this, _cx| {
                        this.state.borrow_mut().select_tool(Tool::Move);
                    },
                ))
                .child(menu_icon_button(
                    "tool-tracks",
                    ICON_TRACKS,
                    tr("toolbar.point_track"),
                    ToolGroup::Tracks,
                    has_track,
                    true,
                    cx,
                ))
                .child(div().w(px(22.)).h(px(1.)).my_1().bg(c(0x1d2a3d)))
                .child(group_icon_button(
                    "tool-calibration",
                    ICON_RULER,
                    tr("toolbar.new_stick"),
                    ToolGroup::Calibration,
                    matches!(selection, Selection::Stick(_)) || tool.is_calibration_group(),
                    true,
                    cx,
                    |this, _cx| {
                        this.state.borrow_mut().add_calibration_stick();
                    },
                ))
                .child(group_icon_button(
                    "tool-measurement",
                    measurement_tool.icon(),
                    tr("toolbar.measure"),
                    ToolGroup::Measurement,
                    tool.is_measurement_group(),
                    true,
                    cx,
                    move |this, _cx| {
                        this.state.borrow_mut().select_tool(measurement_tool);
                    },
                ))
                .child(div().w(px(22.)).h(px(1.)).my_1().bg(c(0x1d2a3d)))
                .child(icon_button(
                    "tool-autostep",
                    ICON_STEP,
                    autostep_tip,
                    32.0,
                    18.0,
                    auto_advance,
                    has_track,
                    cx,
                    move |this, _cx| {
                        let mut st = this.state.borrow_mut();
                        if let Some(track) = st.active_track_mut() {
                            track.auto_advance = !auto_advance;
                            st.commit_edit();
                        }
                    },
                ))
                .child(point_group)
                .child(rigid_group)
                .child(div().w(px(22.)).h(px(1.)).my_1().bg(c(0x1d2a3d)))
                .child(icon_button(
                    "tool-axes",
                    ICON_AXES,
                    tr("toolbar.axes"),
                    32.0,
                    18.0,
                    axes_visible,
                    true,
                    cx,
                    move |this, _cx| {
                        let mut st = this.state.borrow_mut();
                        st.axes_visible = !axes_visible;
                    },
                ))
        };
        let video = div()
            .flex_1()
            .min_w(px(0.))
            .min_h(px(0.))
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .flex()
                    .child(toolbar)
                    .child(stage),
            )
            .child(video_controls(state.clone(), cx));

        // ---- left panel: workspace / video tree ----
        let left = {
            if !state.borrow().left_open {
                collapsed_rail(true, cx)
            } else {
                let (workdir_name, workdir_path, tree, has_workdir) = {
                    let st = state.borrow();
                    let name = st
                        .workdir
                        .as_ref()
                        .and_then(|path| path.file_name())
                        .and_then(|name| name.to_str())
                        .map(|name| name.to_string())
                        .unwrap_or_else(|| tr("toolbar.no_folder"));
                    let path = st
                        .workdir
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_default();
                    (name, path, st.tree.clone(), st.workdir.is_some())
                };

                let refresh = icon_button(
                    "refresh-workdir",
                    ICON_REFRESH,
                    tr("toolbar.refresh"),
                    24.0,
                    14.0,
                    false,
                    has_workdir,
                    cx,
                    |this, _cx| {
                        let path = this.state.borrow().workdir.clone();
                        if let Some(path) = path {
                            this.state.borrow_mut().set_workdir(path);
                        }
                    },
                );

                let mut tree_row_id = 0usize;
                let rows = tree_rows(state.clone(), &tree, 0, &mut tree_row_id, cx);
                let tree_body: AnyElement = if tree.is_empty() {
                    div()
                        .text_xs()
                        .text_color(c(0x5f6f88))
                        .child(if has_workdir {
                            tr("toolbar.no_videos")
                        } else {
                            String::new()
                        })
                        .into_any_element()
                } else {
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .flex_1()
                        .min_h(px(0.))
                        .overflow_y_scrollbar()
                        .children(rows)
                        .into_any_element()
                };

                div()
                    .w(px(260.))
                    .flex_none()
                    .h_full()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .bg(c(0x0d1420))
                    .border_r_1()
                    .border_color(c(0x1d2a3d))
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .truncate()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(c(0xb9cbe4))
                                    .child(workdir_name),
                            )
                            .child(refresh)
                            .child(icon_button(
                                "collapse-left",
                                ICON_BACK1,
                                tr("toolbar.collapse_workspace"),
                                20.0,
                                12.0,
                                false,
                                true,
                                cx,
                                |this, _cx| {
                                    this.state.borrow_mut().left_open = false;
                                },
                            )),
                    )
                    .child(
                        div()
                            .w_full()
                            .truncate()
                            .text_xs()
                            .text_color(c(0x5f6f88))
                            .child(workdir_path),
                    )
                    .child(tree_body)
                    .into_any_element()
            }
        };

        // ---- right panel: properties + data table ----
        let right = {
            if !state.borrow().right_open {
                collapsed_rail(false, cx)
            } else {
                let st = state.borrow();
                let track = st.tracks.get(st.active);
                let mut track_list = div().flex().flex_col().gap_1();
                for (i, t) in st.tracks.iter().enumerate() {
                    let active = i == st.active;
                    track_list = track_list.child(
                        div()
                            .id(("track", i))
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(if active { c(0x152238) } else { c(0x0d1420) })
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _ev, _window, cx| {
                                    let mut st = this.state.borrow_mut();
                                    st.active = i;
                                    if let Some(track) = st.tracks.get(i) {
                                        st.selection = Selection::Track(track.id);
                                    }
                                    st.sync_plot_for_active();
                                    drop(st);
                                    cx.notify();
                                }),
                            )
                            .child(div().w(px(8.)).h(px(8.)).rounded_sm().bg(rgb(t.color)))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .truncate()
                                    .text_xs()
                                    .child(t.name.clone()),
                            )
                            .child(div().text_xs().text_color(c(0x5f6f88)).child(
                                if t.is_rigid_body() {
                                    tr("track.kind.rigid")
                                } else if t.is_point_mass() {
                                    format!("{}·{}", t.footprint.label(), t.step_size)
                                } else {
                                    t.kind_label().to_string()
                                },
                            ))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(c(0x7d8ca6))
                                    .child(t.step_count().to_string()),
                            ),
                    );
                }
                let mut rows = div().flex().flex_col().gap_1();
                let table_decimals = st.calibration.decimals;
                let frame_times = st
                    .video
                    .as_ref()
                    .map(|video| video.meta.frame_times.as_slice())
                    .unwrap_or(&[]);
                let format_value = |value: Option<f32>| {
                    value
                        .filter(|value| value.is_finite())
                        .map(|value| format!("{:.*}", table_decimals, value))
                        .unwrap_or_else(|| "-".to_string())
                };
                let columns = st.table_columns;
                let mut table_header = div()
                    .flex()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .flex_none()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(c(0x8fa3bf));
                if columns[0] {
                    table_header = table_header.child(div().w(px(48.)).child("t (s)"));
                }
                if columns[1] {
                    table_header = table_header.child(div().w(px(58.)).child("x"));
                }
                if columns[2] {
                    table_header = table_header.child(div().w(px(58.)).child("y"));
                }
                if columns[3] {
                    table_header = table_header.child(div().w(px(60.)).child("vx"));
                }
                if columns[4] {
                    table_header = table_header.child(div().w(px(60.)).child("vy"));
                }
                let mut table_rows = div().flex().flex_col().gap_0p5();
                let mut row_count = 0usize;
                let mut selected_count = 0usize;
                let table_visible = st.right_view == RightView::Plot
                    && !st.collapsed.contains(&SideSection::Data)
                    && track.map_or(false, |track| track.is_point_mass());
                if table_visible {
                    if let Some(track) = track.filter(|track| track.is_point_mass()) {
                        for (frame, image_x, image_y) in track.points() {
                            row_count += 1;
                            let selected = st.table_selection.contains(&frame);
                            if selected {
                                selected_count += 1;
                            }
                            let (world_x, world_y) = track
                                .world_position(frame, &st.coords)
                                .unwrap_or_else(|| st.to_world(image_x, image_y));
                            let velocity = track.world_velocity(frame, &st.coords, frame_times);
                            let on = frame == st.frame;
                            let mut row = div()
                                .id(("table-row", frame))
                                .flex()
                                .gap_2()
                                .px_2()
                                .py_0p5()
                                .rounded_sm()
                                .cursor_pointer()
                                .bg(if selected {
                                    c(0x1b2b45)
                                } else if on {
                                    c(0x16233a)
                                } else {
                                    c(0x0d1420)
                                })
                                .text_xs()
                                .text_color(if selected || on {
                                    c(0xdbe6f5)
                                } else {
                                    c(0xa9bad2)
                                })
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _ev, _window, cx| {
                                        let mut st = this.state.borrow_mut();
                                        if !st.table_selection.remove(&frame) {
                                            st.table_selection.insert(frame);
                                        }
                                        drop(st);
                                        cx.notify();
                                    }),
                                );
                            if columns[0] {
                                row = row.child(
                                    div()
                                        .w(px(48.))
                                        .child(format!("{:.3}", st.frame_time(frame))),
                                );
                            }
                            if columns[1] {
                                row = row.child(
                                    div()
                                        .w(px(58.))
                                        .child(format!("{:.*}", table_decimals, world_x)),
                                );
                            }
                            if columns[2] {
                                row = row.child(
                                    div()
                                        .w(px(58.))
                                        .child(format!("{:.*}", table_decimals, world_y)),
                                );
                            }
                            if columns[3] {
                                row = row.child(
                                    div()
                                        .w(px(60.))
                                        .child(format_value(velocity.map(|value| value.0))),
                                );
                            }
                            if columns[4] {
                                row = row.child(
                                    div()
                                        .w(px(60.))
                                        .child(format_value(velocity.map(|value| value.1))),
                                );
                            }
                            table_rows = table_rows.child(row);
                        }
                    }
                }
                rows = rows.child(table_header).child(table_rows);
                let empty = track.map_or(true, |track| track.step_count() == 0);
                let size = if st.img_w > 0.0 {
                    format!("{}×{}", st.img_w as u32, st.img_h as u32)
                } else {
                    "-".to_string()
                };
                let fps = st.video.as_ref().map(|video| video.meta.fps).unwrap_or(0.0);
                let unit = st.calibration.unit.clone();
                let decimals = st.calibration.decimals;
                let angle = st.coords.angle(st.frame).to_degrees();
                let origin = st.coords.origin(st.frame);
                let activity = st
                    .tracking
                    .as_ref()
                    .map(|run| {
                        (
                            t!(
                                "status.tracking",
                                frame = run.last_frame,
                                points = run.points,
                                confidence = format_confidence(run.last_confidence)
                            )
                            .to_string(),
                            true,
                        )
                    })
                    .or_else(|| st.status.clone().map(|status| (status, false)));

                let mut object_rows: Vec<AnyElement> = Vec::new();
                match st.selection {
                    Selection::Axes => {
                        object_rows.push(kv(tr("object.object"), tr("object.axes")).into_any_element());
                        object_rows.push(editable_value(
                            state.clone(),
                            cx,
                            tr("object.origin_x"),
                            InlineTarget::OriginX,
                            format!("{:.2}", origin[0]),
                            " px".to_string(),
                            230,
                        ));
                        object_rows.push(editable_value(
                            state.clone(),
                            cx,
                            tr("object.origin_y"),
                            InlineTarget::OriginY,
                            format!("{:.2}", origin[1]),
                            " px".to_string(),
                            231,
                        ));
                        object_rows.push(editable_value(
                            state.clone(),
                            cx,
                            tr("object.angle"),
                            InlineTarget::Angle,
                            format!("{angle:.2}"),
                            " °".to_string(),
                            232,
                        ));
                        object_rows.push(
                            kv(tr("object.scale"), format!("{:.2} px/{}", st.ppm(), unit)).into_any_element(),
                        );
                        let axes_visible = st.axes_visible;
                        object_rows.push(check_item(
                            200,
                            tr("object.visible"),
                            axes_visible,
                            true,
                            cx,
                            move |this, _cx| {
                                let mut st = this.state.borrow_mut();
                                st.axes_visible = !axes_visible;
                            },
                        ));
                        let locked = st.coords.locked;
                        object_rows.push(check_item(
                            201,
                            tr("object.locked"),
                            locked,
                            true,
                            cx,
                            move |this, _cx| {
                                let mut st = this.state.borrow_mut();
                                st.coords.locked = !locked;
                                st.commit_edit();
                            },
                        ));
                        let fixed_origin = st.coords.fixed_origin;
                        object_rows.push(check_item(
                            202,
                            tr("object.fixed_origin"),
                            fixed_origin,
                            !locked,
                            cx,
                            move |this, _cx| {
                                let mut st = this.state.borrow_mut();
                                let frame = st.frame;
                                st.coords.set_fixed_origin(!fixed_origin, frame);
                                st.commit_edit();
                            },
                        ));
                        let fixed_angle = st.coords.fixed_angle;
                        object_rows.push(check_item(
                            203,
                            tr("object.fixed_angle"),
                            fixed_angle,
                            !locked,
                            cx,
                            move |this, _cx| {
                                let mut st = this.state.borrow_mut();
                                let frame = st.frame;
                                st.coords.set_fixed_angle(!fixed_angle, frame);
                                st.commit_edit();
                            },
                        ));
                    }
                    Selection::Stick(index) => {
                        if let Some((_, _, length)) = st.calibration.stick(st.frame, index) {
                            let visible = st.calibration.stick_visible(index);
                            object_rows.push(
                                kv(tr("object.object"), t!("object.stick", n = index + 1).to_string()).into_any_element(),
                            );
                            object_rows.push(editable_value(
                                state.clone(),
                                cx,
                                tr("object.length"),
                                InlineTarget::StickLength(index),
                                format!("{:.*}", decimals, length),
                                format!(" {}", unit),
                                233,
                            ));
                            object_rows.push(check_item(
                                211,
                                tr("object.visible"),
                                visible,
                                true,
                                cx,
                                move |this, _cx| {
                                    let mut st = this.state.borrow_mut();
                                    st.calibration.set_stick_visible(index, !visible);
                                    st.commit_edit();
                                },
                            ));
                            object_rows.push(plot_chip(
                                212,
                                tr("object.unit_decimals"),
                                false,
                                cx,
                                move |this, _cx| {
                                    let mut st = this.state.borrow_mut();
                                    st.active_stick = index;
                                    st.open_calibration_dialog();
                                },
                            ));
                        }
                    }
                    Selection::Track(id) => {
                        if let Some(track) = st.tracks.iter().find(|track| track.id == id) {
                            let visible = track.visible;
                            let locked = track.locked;
                            let is_point_mass = track.is_point_mass();
                            object_rows.push(
                                kv(tr("object.object"), format!("{} · {}", track.name, track.kind_label()))
                                    .into_any_element(),
                            );
                            object_rows.push(check_item(
                                220,
                                tr("object.visible"),
                                visible,
                                true,
                                cx,
                                move |this, _cx| {
                                    let mut st = this.state.borrow_mut();
                                    if let Some(track) =
                                        st.tracks.iter_mut().find(|track| track.id == id)
                                    {
                                        track.visible = !visible;
                                        st.commit_edit();
                                    }
                                },
                            ));
                            object_rows.push(check_item(
                                221,
                                tr("object.locked"),
                                locked,
                                true,
                                cx,
                                move |this, _cx| {
                                    let mut st = this.state.borrow_mut();
                                    if let Some(track) =
                                        st.tracks.iter_mut().find(|track| track.id == id)
                                    {
                                        track.locked = !locked;
                                        st.commit_edit();
                                    }
                                },
                            ));
                            if is_point_mass {
                                if let Some(rigid) = &track.rigid {
                                    let markerless = rigid.markers.is_empty();
                                    let source = match rigid.dictionary.as_str() {
                                        "ORB" => tr("object.markerless_orb"),
                                        "MOTION" => t!(
                                            "object.motion_contour",
                                            n = rigid.mask_mode.label()
                                        )
                                        .to_string(),
                                        _ => t!(
                                            "object.markers",
                                            kind = rigid.dictionary,
                                            n = rigid.markers.len()
                                        )
                                        .to_string(),
                                    };
                                    object_rows.push(
                                        kv(
                                            if markerless {
                                                tr("object.tracking")
                                            } else {
                                                "ArUco".to_string()
                                            },
                                            source,
                                        )
                                        .into_any_element(),
                                    );
                                    if let Some(step) = track.rigid_step(st.frame) {
                                        object_rows.push(
                                            kv(
                                                tr("object.rigid_angle"),
                                                format!("{:.2}°", step.angle.to_degrees()),
                                            )
                                            .into_any_element(),
                                        );
                                        object_rows.push(
                                            kv(tr("object.feature_size"), format!("{:.1} px", step.scale))
                                                .into_any_element(),
                                        );
                                        let quality_label = match rigid.dictionary.as_str() {
                                            "ORB" => tr("object.inliers"),
                                            "MOTION" => tr("object.match_degree"),
                                            _ => tr("object.visible_markers"),
                                        };
                                        object_rows.push(
                                            kv(quality_label, format!("{}", step.marker_count))
                                                .into_any_element(),
                                        );
                                    }
                                    if !rigid.contour.is_empty() {
                                        object_rows.push(
                                            kv(tr("object.outline_points"), format!("{}", rigid.contour.len()))
                                                .into_any_element(),
                                        );
                                    }
                                }
                                if let Some(velocity) =
                                    track.world_velocity(st.frame, &st.coords, frame_times)
                                {
                                    object_rows.push(
                                        kv(
                                            tr("object.velocity"),
                                            format!(
                                                "({:.2}, {:.2}) {}/s",
                                                velocity.0, velocity.1, unit
                                            ),
                                        )
                                        .into_any_element(),
                                    );
                                }
                                if let Some(acceleration) =
                                    track.world_acceleration(st.frame, &st.coords, frame_times)
                                {
                                    object_rows.push(
                                        kv(
                                            tr("object.acceleration"),
                                            format!(
                                                "({:.2}, {:.2}) {}/s²",
                                                acceleration.0, acceleration.1, unit
                                            ),
                                        )
                                        .into_any_element(),
                                    );
                                }
                            } else if let Some(measurement_rows) =
                                measurement_summary(track, st.frame, &st.coords, &unit, decimals)
                            {
                                for (label, value) in measurement_rows {
                                    object_rows.push(kv(label, value).into_any_element());
                                }
                            }
                        }
                    }
                    Selection::None => {}
                }

                let collapsed_properties = st.collapsed.contains(&SideSection::Properties);
                let collapsed_tracks = st.collapsed.contains(&SideSection::Tracks);
                let collapsed_object = st.collapsed.contains(&SideSection::Object);
                let collapsed_data = st.collapsed.contains(&SideSection::Data);
                let collapsed_plot = st.collapsed.contains(&SideSection::Plot);
                let show_data = track.map_or(true, |track| track.is_point_mass());
                let has_plot = track.map_or(false, |track| track.is_point_mass());
                let is_point_mass = track.map_or(false, |track| track.is_point_mass());
                let right_view = st.right_view;
                div()
                    .w(px(400.))
                    .flex_none()
                    .h_full()
                    .min_h(px(0.))
                    .overflow_y_scrollbar()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .bg(c(0x0d1420))
                    .border_l_1()
                    .border_color(c(0x1d2a3d))
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(c(0xb9cbe4))
                                    .child(tr("inspector.title")),
                            )
                            .child(plot_chip(
                                500,
                                tr("inspector.object_properties"),
                                right_view == RightView::Inspector,
                                cx,
                                |this, _cx| {
                                    this.state.borrow_mut().right_view = RightView::Inspector;
                                },
                            ))
                            .child(plot_chip(
                                501,
                                tr("inspector.tab_plot"),
                                right_view == RightView::Plot,
                                cx,
                                |this, _cx| {
                                    this.state.borrow_mut().right_view = RightView::Plot;
                                },
                            ))
                            .child(icon_button(
                                "collapse-right",
                                ICON_FWD1,
                                tr("inspector.collapse"),
                                20.0,
                                12.0,
                                false,
                                true,
                                cx,
                                |this, _cx| {
                                    this.state.borrow_mut().right_open = false;
                                },
                            )),
                    )
                    .when(right_view == RightView::Inspector, |this| {
                        this.child(section_header(
                            tr("inspector.properties"),
                            SideSection::Properties,
                            state.clone(),
                            cx,
                        ))
                        .when(!collapsed_properties, |this| {
                            this.child(kv(tr("inspector.file"), st.title.clone()))
                                .child(kv(tr("inspector.size"), size))
                                .child(kv(tr("inspector.frame"), format!("{} / {:.1}fps", st.frame_count, fps)))
                                .child(kv(
                                    tr("inspector.time"),
                                    format!(
                                        "{} / {}",
                                        format_time(st.current_time()),
                                        format_time(st.total_time())
                                    ),
                                ))
                        })
                        .child(section_header(
                            tr("inspector.tracks"),
                            SideSection::Tracks,
                            state.clone(),
                            cx,
                        ))
                        .when(!collapsed_tracks, |this| {
                            this.child(if st.tracks.is_empty() {
                                div().text_xs().text_color(c(0x5f6f88)).child(tr("inspector.no_tracks"))
                            } else {
                                track_list
                            })
                        })
                        .when(st.selection != Selection::None, |this| {
                            this.child(section_header(
                                tr("inspector.object_properties"),
                                SideSection::Object,
                                state.clone(),
                                cx,
                            ))
                            .when(!collapsed_object, |this| this.children(object_rows))
                        })
                    })
                    .when(right_view == RightView::Plot, |this| {
                        this.when(has_plot, |this| {
                            this.child(section_header(tr("inspector.plot"), SideSection::Plot, state.clone(), cx))
                                .when(!collapsed_plot, |this| {
                                    this.child(plot_sidebar(state.clone(), cx))
                                })
                        })
                        .when(show_data, |this| {
                            this.child(section_header(
                                tr("inspector.data_table"),
                                SideSection::Data,
                                state.clone(),
                                cx,
                            ))
                            .when(!collapsed_data, |this| {
                                let mut columns_row =
                                    div().flex().items_center().gap_2().child(
                                        div().text_xs().text_color(c(0x7d8ca6)).child(tr("inspector.columns")),
                                    );
                                for (index, label) in
                                    ["t", "x", "y", "vx", "vy"].into_iter().enumerate()
                                {
                                    let checked = st.table_columns[index];
                                    columns_row = columns_row.child(check_item(
                                        600 + index,
                                        label,
                                        checked,
                                        true,
                                        cx,
                                        move |this, _cx| {
                                            let mut st = this.state.borrow_mut();
                                            st.table_columns[index] = !st.table_columns[index];
                                        },
                                    ));
                                }
                                this.child(columns_row)
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(div().text_xs().text_color(c(0x5f6f88)).child(
                                                t!(
                                                    "inspector.rows",
                                                    rows = row_count,
                                                    selected = selected_count
                                                )
                                                .to_string(),
                                            ))
                                            .child(div().flex_1())
                                            .child(plot_chip(
                                                610,
                                                tr("inspector.select_all"),
                                                false,
                                                cx,
                                                |this, _cx| {
                                                    let mut st = this.state.borrow_mut();
                                                    let frames: HashSet<usize> = st
                                                        .active_track()
                                                        .filter(|track| track.is_point_mass())
                                                        .map(|track| {
                                                            track
                                                                .points()
                                                                .map(|(frame, _, _)| frame)
                                                                .collect()
                                                        })
                                                        .unwrap_or_default();
                                                    st.table_selection = frames;
                                                },
                                            ))
                                            .child(plot_chip(
                                                611,
                                                tr("inspector.clear"),
                                                false,
                                                cx,
                                                |this, _cx| {
                                                    this.state.borrow_mut().table_selection.clear();
                                                },
                                            ))
                                            .child(plot_chip(
                                                612,
                                                tr("inspector.copy"),
                                                false,
                                                cx,
                                                |this, cx| {
                                                    let text = this.state.borrow().table_tsv();
                                                    if !text.is_empty() {
                                                        cx.write_to_clipboard(
                                                            ClipboardItem::new_string(text),
                                                        );
                                                        this.state.borrow_mut().status =
                                                            Some(tr("status.table_copied"));
                                                    }
                                                },
                                            )),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(c(0x5f6f88))
                                            .child(tr("inspector.table_hint")),
                                    )
                                    .child(if empty {
                                        div()
                                            .text_xs()
                                            .text_color(c(0x5f6f88))
                                            .child(tr("inspector.no_points"))
                                    } else {
                                        rows
                                    })
                            })
                        })
                        .when(!is_point_mass, |this| {
                            this.child(
                                div()
                                    .text_xs()
                                    .text_color(c(0x5f6f88))
                                    .child(tr("inspector.plot_only_point")),
                            )
                        })
                    })
                    .child(div().flex_1())
                    .when_some(activity, |this, (status, running)| {
                        this.child(
                            div()
                                .truncate()
                                .text_xs()
                                .text_color(if running { c(0xffd43b) } else { c(0x9fb2cc) })
                                .child(status),
                        )
                    })
                    .into_any_element()
            }
        };

        let has_doc = {
            let st = state.borrow();
            !st.is_placeholder()
        };
        let center = if has_doc {
            div()
                .flex_1()
                .min_w(px(0.))
                .min_h(px(0.))
                .h_full()
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(tab_bar(state.clone(), cx))
                .child(video)
        } else {
            div()
                .flex_1()
                .min_w(px(0.))
                .min_h(px(0.))
                .h_full()
                .overflow_hidden()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .child(div().text_sm().text_color(c(0x5f6f88)).child(tr("inspector.no_video")))
        };

        // ---- bottom ----
        let bottom = {
            let st = state.borrow();
            div()
                .h(px(30.))
                .flex()
                .items_center()
                .gap_5()
                .px_3()
                .bg(c(0x0a1018))
                .border_t_1()
                .border_color(c(0x1d2a3d))
                .text_xs()
                .text_color(c(0x7d8ca6))
                .child(
                    div()
                        .text_color(if st.fps >= 55.0 {
                            c(0x51cf66)
                        } else if st.fps >= 30.0 {
                            c(0xffd43b)
                        } else {
                            c(0xff6b6b)
                        })
                        .child(if st.playing {
                            format!("{:.1} fps", st.fps)
                        } else {
                            "idle".to_string()
                        }),
                )
                .child(format!("draw {:.2} ms", st.draw_ms))
                .child(format!("points {}", st.drawn_points))
                .child(format!("zoom {:.0}%", st.scale * 100.0))
                .child(format!(
                    "frame {} / {}",
                    st.frame,
                    st.frame_count.saturating_sub(1)
                ))
                .child(format!("t {:.3}s", st.current_time()))
                .child(format!("track {}", st.active + 1))
                .child(format!("tool {}", st.tool.label()))
                .when_some(
                    st.tracking
                        .as_ref()
                        .map(|run| {
                            t!(
                                "status.tracking",
                                frame = run.last_frame,
                                points = run.points,
                                confidence = format_confidence(run.last_confidence)
                            )
                            .to_string()
                        })
                        .or_else(|| st.status.clone()),
                    |this, status| {
                        this.child(
                            div()
                                .truncate()
                                .text_color(if st.tracking.is_some() {
                                    c(0xffd43b)
                                } else {
                                    c(0x9fb2cc)
                                })
                                .child(status),
                        )
                    },
                )
        };

        let title_bar = TitleBar::new()
            .bg(c(0x070b12))
            .border_color(c(0x1d2a3d))
            .text_color(c(0xdbe6f5))
            .child(menu_bar(state.clone(), cx));
        let menu_overlay = if state.borrow().menu_open.is_some()
            || state.borrow().tool_menu.is_some()
            || state.borrow().plot_var_menu.is_some()
        {
            // Keep the menu bar available when switching between its menus.
            let top = if state.borrow().tool_menu.is_some()
                || state.borrow().plot_var_menu.is_some()
            {
                px(0.)
            } else {
                TITLE_BAR_HEIGHT
            };
            crate::overlay::overlay_surface()
                .absolute()
                .top(top)
                .bottom(px(0.))
                .left(px(0.))
                .right(px(0.))
                .bg(transparent_black())
                .on_any_mouse_down(cx.listener(|this, _ev, _window, cx| {
                    let mut st = this.state.borrow_mut();
                    st.menu_open = None;
                    st.tool_menu = None;
                    st.plot_var_menu = None;
                    drop(st);
                    cx.notify();
                }))
                .into_any_element()
        } else {
            div().into_any_element()
        };
        let popup = menu_popup(state.clone(), cx);
        let tool_popup = tool_menu_popup(state.clone(), cx);
        let plot_var_popup = plot_var_popup(state.clone(), cx);
        let modal = modal_view(state.clone(), cx);

        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(c(0x070b12))
            .text_color(c(0xdbe6f5))
            .child(title_bar)
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .w_full()
                    .overflow_hidden()
                    .flex()
                    .child(left)
                    .child(center)
                    .child(right),
            )
            .child(bottom)
            .child(menu_overlay)
            .child(popup)
            .child(tool_popup)
            .child(plot_var_popup)
            .child(modal)
            .into_any_element()
    }
}

impl Probe {
    fn render_tracker_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if !self.focused_once {
            self.focused_once = true;
            window.focus(&self.focus, cx);
        }
        if !self.close_hook {
            self.close_hook = true;
            if !self.owner_applied {
                self.owner_applied = true;
                if let (Some(owner), Some(hwnd)) = (self.owner_hwnd, native_hwnd(window)) {
                    set_native_owner(hwnd, owner);
                }
            }
            let state = self.state.clone();
            let main = self.main.clone();
            window.on_window_should_close(cx, move |_window, cx| {
                state.borrow_mut().close_tracker_dialog();
                if let Some(main) = &main {
                    let _ = main.update(cx, |_probe, cx| cx.notify());
                }
                true
            });
        }
        let open = self.state.borrow().tracker_open;
        if !open {
            if let Some(main) = &self.main {
                let _ = main.update(cx, |_probe, cx| cx.notify());
            }
            window.remove_window();
            return div().into_any_element();
        }
        window.request_animation_frame();
        let drops: Vec<_> = self
            .state
            .borrow_mut()
            .tracker_thumbs_drop
            .drain(..)
            .collect();
        for image in drops {
            let _ = window.drop_image(image);
        }
        div()
            .size_full()
            .bg(c(0x070b12))
            .text_color(c(0xdbe6f5))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _window, cx| {
                this.tracker_dialog_key(ev, cx);
            }))
            .child(crate::tracker_modal::tracker_settings_modal_view(
                self.state.clone(),
                cx,
            ))
            .into_any_element()
    }

    fn tracker_dialog_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let mut st = self.state.borrow_mut();
        match ev.keystroke.key.as_str() {
            "escape" => st.close_tracker_dialog(),
            "enter" => {
                st.apply_tracker_dialog();
            }
            "tab" => st.cycle_tracker_field(),
            "backspace" => {
                let field = st.tracker_field;
                st.tracker_input_mut(field).pop();
            }
            key => {
                let mut chars = key.chars();
                if chars.clone().count() == 1
                    && !ev.keystroke.modifiers.control
                    && !ev.keystroke.modifiers.platform
                {
                    let ch = chars.next().unwrap();
                    if ch.is_ascii_digit() || ch == '.' {
                        let field = st.tracker_field;
                        st.tracker_input_mut(field).push(ch);
                    }
                }
            }
        }
        drop(st);
        cx.notify();
    }
}
