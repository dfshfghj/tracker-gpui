use std::cell::RefCell;
use std::rc::Rc;

use gpui_component::TITLE_BAR_HEIGHT;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use rust_i18n::t;

use crate::i18n::tr;
use crate::*;

/// `[x] Label` / `[ ] Label`, used by every checkbox-style menu item.
pub(crate) fn check(label: String, on: bool) -> String {
    t!(
        "menu.check",
        label = label,
        mark = if on { "x" } else { " " }
    )
    .to_string()
}

pub(crate) fn menu_item(
    label: impl Into<String>,
    enabled: bool,
    cx: &mut Context<Probe>,
    action: impl Fn(&mut Probe, &mut Context<Probe>) + 'static,
) -> AnyElement {
    let label = label.into();
    div()
        .id(label.clone())
        .w_full()
        .flex()
        .items_center()
        .h(px(26.))
        .px_3()
        .text_xs()
        .text_color(if enabled { c(0xdbe6f5) } else { c(0x5f6f88) })
        .when(enabled, |this| {
            this.cursor_pointer().on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _ev, _window, cx| {
                    {
                        let mut st = this.state.borrow_mut();
                        st.menu_open = None;
                    }
                    action(this, cx);
                    cx.notify();
                }),
            )
        })
        .child(label)
        .into_any_element()
}

pub(crate) fn menu_item_dynamic(
    label: String,
    id: usize,
    enabled: bool,
    cx: &mut Context<Probe>,
    action: impl Fn(&mut Probe, &mut Context<Probe>) + 'static,
) -> AnyElement {
    div()
        .id(("dynamic-menu-item", id))
        .w_full()
        .flex()
        .items_center()
        .h(px(26.))
        .px_3()
        .text_xs()
        .truncate()
        .text_color(if enabled { c(0xdbe6f5) } else { c(0x5f6f88) })
        .when(enabled, |this| {
            this.cursor_pointer().on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _ev, _window, cx| {
                    {
                        let mut st = this.state.borrow_mut();
                        st.menu_open = None;
                    }
                    action(this, cx);
                    cx.notify();
                }),
            )
        })
        .child(label)
        .into_any_element()
}

pub(crate) fn menu_separator() -> AnyElement {
    div()
        .h(px(1.))
        .mx_2()
        .my_1()
        .bg(c(0x1d2a3d))
        .into_any_element()
}

pub(crate) fn menu_bar(state: Rc<RefCell<State>>, cx: &mut Context<Probe>) -> impl IntoElement {
    let (open, project_label) = {
        let st = state.borrow();
        (
            st.menu_open,
            st.project_path
                .as_ref()
                .and_then(|path| path.file_name())
                .and_then(|name| name.to_str())
                .map(|name| name.to_string())
                .unwrap_or_else(|| tr("app.untitled_project")),
        )
    };
    let entries = [
        (MenuId::File, tr("menu.bar.file")),
        (MenuId::Edit, tr("menu.bar.edit")),
        (MenuId::View, tr("menu.bar.view")),
        (MenuId::Tracks, tr("menu.bar.tracks")),
        (MenuId::Coords, tr("menu.bar.coords")),
        (MenuId::Help, tr("menu.bar.help")),
    ];
    let mut bar = div()
        .h_full()
        .w_full()
        .flex_none()
        .flex()
        .items_center()
        .gap_1()
        .px_2();
    for (index, (id, label)) in entries.into_iter().enumerate() {
        let menu_state = state.clone();
        bar = bar.child(
            div()
                .id(("menu", index))
                .px_2()
                .py_0p5()
                .rounded_sm()
                .text_xs()
                .cursor_pointer()
                .bg(if open == Some(id) {
                    c(0x16233a)
                } else {
                    c(0x000000).alpha(0.0)
                })
                .text_color(if open == Some(id) {
                    c(0xdbe6f5)
                } else {
                    c(0x9fb2cc)
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |_this, ev: &MouseDownEvent, _window, cx| {
                        let mut st = menu_state.borrow_mut();
                        st.menu_open = if st.menu_open == Some(id) {
                            None
                        } else {
                            Some(id)
                        };
                        st.menu_x = f32::from(ev.position.x);
                        drop(st);
                        cx.notify();
                    }),
                )
                .child(label),
        );
    }
    bar.child(div().flex_1())
        .child(
            div()
                .max_w(px(300.))
                .truncate()
                .text_xs()
                .text_color(c(0x5f6f88))
                .child(project_label),
        )
        .child(layout_controls(&state.borrow(), cx))
}

pub(crate) fn tool_menu_item(
    label: String,
    active: bool,
    id: usize,
    cx: &mut Context<Probe>,
    action: impl Fn(&mut Probe, &mut Context<Probe>) + 'static,
) -> AnyElement {
    div()
        .id(("tool-menu-item", id))
        .w_full()
        .flex()
        .items_center()
        .gap_2()
        .h(px(26.))
        .px_3()
        .text_xs()
        .text_color(if active { c(0x8ecbff) } else { c(0xdbe6f5) })
        .cursor_pointer()
        .hover(|style| style.bg(c(0x1d2a3d)))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _ev, _window, cx| {
                {
                    let mut st = this.state.borrow_mut();
                    st.tool_menu = None;
                }
                action(this, cx);
                cx.notify();
            }),
        )
        .child(if active { "●" } else { " " })
        .child(label)
        .into_any_element()
}

pub(crate) fn tool_menu_popup(state: Rc<RefCell<State>>, cx: &mut Context<Probe>) -> AnyElement {
    let (menu, x, y, tool, point_method, rigid_method) = {
        let st = state.borrow();
        (
            st.tool_menu,
            st.tool_menu_pos.0,
            st.tool_menu_pos.1,
            st.tool,
            st.point_detect_method,
            st.rigid_detect_method,
        )
    };
    #[cfg(not(feature = "vision"))]
    let _ = (point_method, rigid_method);
    let Some(menu) = menu else {
        return div().into_any_element();
    };
    let mut items = Vec::new();
    match menu {
        ToolGroup::Calibration => {
            items.push(tool_menu_item(
                tr("menu.tool.set_origin"),
                tool == Tool::Origin,
                1,
                cx,
                |this, _cx| {
                    this.state.borrow_mut().select_tool(Tool::Origin);
                },
            ));
            items.push(tool_menu_item(
                tr("menu.tool.origin_offset"),
                tool == Tool::OriginOffset,
                2,
                cx,
                |this, _cx| {
                    this.state.borrow_mut().select_tool(Tool::OriginOffset);
                },
            ));
            items.push(menu_separator());
            items.push(tool_menu_item(
                tr("menu.tool.show_all_sticks"),
                false,
                3,
                cx,
                |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    st.calibration.show_all_sticks();
                    st.commit_edit();
                },
            ));
        }
        ToolGroup::Measurement => {
            items.push(tool_menu_item(
                tr("menu.tool.tape"),
                tool == Tool::Tape,
                4,
                cx,
                |this, _cx| {
                    this.state.borrow_mut().select_tool(Tool::Tape);
                },
            ));
            items.push(tool_menu_item(
                tr("menu.tool.protractor"),
                tool == Tool::Protractor,
                5,
                cx,
                |this, _cx| {
                    this.state.borrow_mut().select_tool(Tool::Protractor);
                },
            ));
            items.push(tool_menu_item(
                tr("menu.tool.circle"),
                tool == Tool::Circle,
                6,
                cx,
                |this, _cx| {
                    this.state.borrow_mut().select_tool(Tool::Circle);
                },
            ));
        }
        #[cfg(feature = "vision")]
        ToolGroup::PointDetect => {
            items.push(tool_menu_item(
                PointDetectMethod::AutoTracker.label().to_string(),
                point_method == PointDetectMethod::AutoTracker,
                40,
                cx,
                |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    st.point_detect_method = PointDetectMethod::AutoTracker;
                    st.rigid_roi_mode = false;
                    st.motion_roi_mode = false;
                    st.status = Some(
                        t!(
                            "menu.status.point_detect",
                            method = PointDetectMethod::AutoTracker.label()
                        )
                        .to_string(),
                    );
                },
            ));
            items.push(tool_menu_item(
                PointDetectMethod::AutoDetect.label().to_string(),
                point_method == PointDetectMethod::AutoDetect,
                41,
                cx,
                |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    st.point_detect_method = PointDetectMethod::AutoDetect;
                    st.rigid_roi_mode = false;
                    st.motion_roi_mode = false;
                    st.status = Some(
                        t!(
                            "menu.status.point_detect",
                            method = PointDetectMethod::AutoDetect.label()
                        )
                        .to_string(),
                    );
                },
            ));
        }
        #[cfg(feature = "vision")]
        ToolGroup::RigidDetect => {
            items.push(tool_menu_item(
                RigidDetectMethod::Aruco.label().to_string(),
                rigid_method == RigidDetectMethod::Aruco,
                50,
                cx,
                |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    st.rigid_detect_method = RigidDetectMethod::Aruco;
                    st.rigid_roi_mode = false;
                    st.motion_roi_mode = false;
                    st.status = Some(
                        t!(
                            "menu.status.rigid_detect",
                            method = RigidDetectMethod::Aruco.label()
                        )
                        .to_string(),
                    );
                },
            ));
            items.push(tool_menu_item(
                RigidDetectMethod::Markerless.label().to_string(),
                rigid_method == RigidDetectMethod::Markerless,
                51,
                cx,
                |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    st.rigid_detect_method = RigidDetectMethod::Markerless;
                    st.rigid_roi_mode = false;
                    st.motion_roi_mode = false;
                    st.status = Some(
                        t!(
                            "menu.status.rigid_detect",
                            method = RigidDetectMethod::Markerless.label()
                        )
                        .to_string(),
                    );
                },
            ));
            items.push(tool_menu_item(
                RigidDetectMethod::MotionContour.label().to_string(),
                rigid_method == RigidDetectMethod::MotionContour,
                52,
                cx,
                |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    st.rigid_detect_method = RigidDetectMethod::MotionContour;
                    st.rigid_roi_mode = false;
                    st.motion_roi_mode = false;
                    st.status = Some(
                        t!(
                            "menu.status.rigid_detect",
                            method = RigidDetectMethod::MotionContour.label()
                        )
                        .to_string(),
                    );
                },
            ));
        }
        ToolGroup::Tracks => {
            let (point_tracks, active) = {
                let st = state.borrow();
                let point_tracks: Vec<(usize, u64, String, usize)> = st
                    .tracks
                    .iter()
                    .enumerate()
                    .filter(|(_, track)| track.is_point_mass())
                    .map(|(index, track)| (index, track.id, track.name.clone(), track.step_count()))
                    .collect();
                (point_tracks, st.active)
            };
            items.push(tool_menu_item(
                tr("menu.tool.new_point"),
                false,
                90,
                cx,
                |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    st.add_point_mass_track();
                    st.select_tool(Tool::Move);
                },
            ));
            items.push(menu_separator());
            if point_tracks.is_empty() {
                items.push(tool_menu_item(
                    tr("menu.tool.no_point_tracks"),
                    false,
                    99,
                    cx,
                    |_this, _cx| {},
                ));
            } else {
                for (index, id, name, points) in point_tracks {
                    items.push(tool_menu_item(
                        t!("menu.track_points", name = name, points = points).to_string(),
                        index == active,
                        100 + index,
                        cx,
                        move |this, _cx| {
                            let mut st = this.state.borrow_mut();
                            st.active = index;
                            st.selection = Selection::Track(id);
                            st.sync_plot_for_active();
                        },
                    ));
                }
            }
        }
    }
    crate::overlay::overlay_surface()
        .absolute()
        .top(px(y))
        .left(px(x + 8.0))
        .w(px(200.))
        .flex()
        .flex_col()
        .py_1()
        .bg(c(0x152238))
        .border_1()
        .border_color(c(0x2a3b55))
        .rounded_sm()
        .children(items)
        .into_any_element()
}

pub(crate) fn plot_var_popup(state: Rc<RefCell<State>>, menu: Option<(usize, PlotAxis)>, pos: (f32, f32), window: &mut Window, cx: &mut Context<Probe>) -> AnyElement {
    let (menu, x, y, available, current) = {
        let st = state.borrow();
        let Some((plot_index, axis)) = menu else {
            return div().into_any_element();
        };
        let Some(track) = st.active_track() else {
            return div().into_any_element();
        };
        let var = st
            .plot
            .configs
            .get(plot_index)
            .map(|config| match axis {
                PlotAxis::X => config.x_var,
                PlotAxis::Y => config.y_var,
            })
            .unwrap_or(PlotVar::X);
        let variables = crate::variables::Variables::compile(track).ok();
        (
            (plot_index, axis),
            pos.0,
            pos.1,
            PlotVar::all_for(track).into_iter().map(|var|
                (var, variables.as_ref().map_or_else(
                    || var.axis_label_for(track, &st.calibration.unit),
                    |vars| vars.axis_label(var, track, &st.calibration.unit)))).collect::<Vec<_>>(),
            var,
        )
    };
    let (plot_index, axis) = menu;
    let mut items = Vec::new();
    for (index, (var, label)) in available.into_iter().enumerate() {
        let active = var == current;
        items.push(
            div()
                .id(("plot-var-item", index))
                .flex_none()
                .w_full()
                .flex()
                .items_center()
                .gap_2()
                .h(px(24.))
                .px_3()
                .text_xs()
                .text_color(if active { c(0x8ecbff) } else { c(0xdbe6f5) })
                .cursor_pointer()
                .hover(|style| style.bg(c(0x1d2a3d)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _ev, _window, cx| {
                        let mut st = this.state.borrow_mut();
                        st.set_plot_var(plot_index, axis, var);
                        this.plot_var_menu = None;
                        drop(st);
                        cx.refresh_windows();
                    }),
                )
                .child(if active { "●" } else { " " })
                .child(label)
                .into_any_element(),
        );
    }
    let viewport = window.viewport_size();
    let height = (items.len() as f32 * 24. + 10.).min(300.).min((f32::from(viewport.height) - 16.).max(24.));
    let top = y.min((f32::from(viewport.height) - height - 8.).max(8.)).max(8.);
    let left = (x + 8.).min((f32::from(viewport.width) - 218.).max(8.)).max(8.);
    crate::overlay::overlay_surface()
        .id("plot-variable-popup")
        .max_h(px(height))
        .overflow_y_scroll()
        .absolute()
        .top(px(top))
        .left(px(left))
        .w(px(210.))
        .flex()
        .flex_col()
        .py_1()
        .bg(c(0x152238))
        .border_1()
        .border_color(c(0x2a3b55))
        .rounded_sm()
        .children(items)
        .into_any_element()
}

pub(crate) fn menu_popup(state: Rc<RefCell<State>>, cx: &mut Context<Probe>) -> AnyElement {
    let (open, x, has_docs, can_undo, can_redo, active_doc, recent_projects) = {
        let st = state.borrow();
        (
            st.menu_open,
            st.menu_x,
            st.docs.iter().any(|doc| !doc.is_placeholder()),
            st.can_undo(),
            st.can_redo(),
            st.active_doc,
            st.prefs.recent_projects.clone(),
        )
    };
    let Some(open) = open else {
        return div().into_any_element();
    };

    let mut items: Vec<AnyElement> = Vec::new();
    match open {
        MenuId::File => {
            items.push(menu_item(tr("menu.item.new_project"), true, cx, |this, cx| {
                request_new_project(this.state.clone(), cx);
            }));
            items.push(menu_item(tr("menu.item.open_project"), true, cx, |this, cx| {
                request_open_project(this.state.clone(), cx);
            }));
            items.push(menu_item(tr("menu.item.save"), has_docs, cx, |this, cx| {
                request_save(this.state.clone(), SaveAction::None, cx);
            }));
            items.push(menu_item(tr("menu.item.save_as"), has_docs, cx, |this, cx| {
                request_save_as(this.state.clone(), SaveAction::None, cx);
            }));
            items.push(menu_item(tr("menu.item.open_folder"), true, cx, |this, cx| {
                request_choose_folder(this.state.clone(), cx);
            }));
            if !recent_projects.is_empty() {
                items.push(menu_separator());
                for (index, path) in recent_projects.into_iter().take(6).enumerate() {
                    let label = path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .map(|name| name.to_string())
                        .unwrap_or_else(|| tr("app.invalid_name"));
                    let path_for_action = path.clone();
                    items.push(menu_item_dynamic(
                        label,
                        index,
                        true,
                        cx,
                        move |this, cx| {
                            request_open_project_path(
                                this.state.clone(),
                                path_for_action.clone(),
                                cx,
                            );
                        },
                    ));
                }
            }
            if has_docs {
                items.push(menu_separator());
                items.push(menu_item(
                    tr("menu.item.close_tab"),
                    true,
                    cx,
                    move |this, cx| {
                        request_close_doc(this.state.clone(), active_doc, cx);
                    },
                ));
            }
        }
        MenuId::Edit => {
            items.push(menu_item(tr("menu.item.undo"), can_undo, cx, |this, _cx| {
                this.state.borrow_mut().undo_edit();
            }));
            items.push(menu_item(
                tr("menu.item.redo"),
                can_redo,
                cx,
                |this, _cx| {
                    this.state.borrow_mut().redo_edit();
                },
            ));
        }
        MenuId::View => {
            items.push(menu_item(tr("menu.item.fit"), has_docs, cx, |this, _cx| {
                this.state.borrow_mut().fit();
            }));
            items.push(menu_separator());
            items.push(menu_item(tr("menu.item.settings"), true, cx, |this, cx| {
                this.open_settings_window(cx);
            }));
            items.push(menu_separator());
            let axes_visible = state.borrow().axes_visible;
            items.push(menu_item_dynamic(
                check(tr("menu.item.axes"), axes_visible),
                43,
                has_docs,
                cx,
                move |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    st.axes_visible = !axes_visible;
                },
            ));
            let (trail_visible, trail_length, labels_visible, trace_visible) = {
                let st = state.borrow();
                (
                    st.trail_visible,
                    st.trail_length,
                    st.labels_visible,
                    st.trace_visible,
                )
            };
            items.push(menu_item_dynamic(
                check(tr("menu.item.trail"), trail_visible),
                46,
                has_docs,
                cx,
                move |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    st.trail_visible = !trail_visible;
                },
            ));
            let trail_label = match trail_length {
                0 => tr("menu.trail.all"),
                4 => tr("menu.trail.short"),
                15 => tr("menu.trail.long"),
                n => t!("menu.trail.frames", n = n).to_string(),
            };
            let next_trail = match trail_length {
                15 => 4,
                4 => 0,
                _ => 15,
            };
            items.push(menu_item_dynamic(
                t!("menu.item.trail_length", label = trail_label).to_string(),
                47,
                has_docs,
                cx,
                move |this, _cx| {
                    this.state.borrow_mut().trail_length = next_trail;
                },
            ));
            items.push(menu_item_dynamic(
                check(tr("menu.item.labels"), labels_visible),
                48,
                has_docs,
                cx,
                move |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    st.labels_visible = !labels_visible;
                },
            ));
            items.push(menu_item_dynamic(
                check(tr("menu.item.trace"), trace_visible),
                49,
                has_docs,
                cx,
                move |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    st.trace_visible = !trace_visible;
                },
            ));
            items.push(menu_separator());
            let (left_open, right_open) = {
                let st = state.borrow();
                (st.left_open, st.right_open)
            };
            items.push(menu_item_dynamic(
                format!("{}  {}", check(tr("menu.item.workspace"), left_open), sidebar_shortcut(true)),
                44,
                true,
                cx,
                move |this, _cx| {
                    this.state.borrow_mut().toggle_sidebar(true);
                },
            ));
            items.push(menu_item_dynamic(
                format!("{}  {}", check(tr("menu.item.inspector"), right_open), sidebar_shortcut(false)),
                45,
                true,
                cx,
                move |this, _cx| {
                    this.state.borrow_mut().toggle_sidebar(false);
                },
            ));
        }
        MenuId::Tracks => {
            let (
                has_track,
                track_name,
                visible,
                locked,
                auto_advance,
                footprint,
                step_size,
                tracking,
                has_seed,
                can_start,
                has_video,
                auto_detect_running,
                motion_pure_mask,
            ) = {
                let st = state.borrow();
                let track = st.tracks.get(st.active);
                (
                    track.is_some(),
                    track.map(|track| track.name.clone()).unwrap_or_default(),
                    track.map(|track| track.visible).unwrap_or(false),
                    track.map(|track| track.locked).unwrap_or(false),
                    track.map(|track| track.auto_advance).unwrap_or(false),
                    track.map(|track| track.footprint).unwrap_or_default(),
                    track.map(|track| track.step_size).unwrap_or(1),
                    st.tracking.is_some(),
                    track.map_or(false, |track| track.has_point(st.frame)),
                    track.map_or(false, |track| {
                        track.is_point_mass() && !track.locked && track.has_point(st.frame)
                    }) && !st.tracking.is_some()
                        && st.frame < st.frame_count.saturating_sub(1),
                    st.video.is_some(),
                    st.auto_detect_running,
                    st.motion_pure_mask,
                )
            };
            #[cfg(not(feature = "vision"))]
            let _ = auto_detect_running;
            items.push(menu_item(
                tr("menu.item.new_point_track"),
                has_video && !tracking,
                cx,
                |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    st.add_point_mass_track();
                    st.tool = Tool::Move;
                },
            ));
            for (id, kind, label) in [
                (30, MeasurementKind::Tape, tr("menu.item.new_tape")),
                (31, MeasurementKind::Protractor, tr("menu.item.new_protractor")),
                (32, MeasurementKind::Circle, tr("menu.item.new_circle")),
            ] {
                items.push(menu_item_dynamic(
                    label,
                    id,
                    has_video && !tracking,
                    cx,
                    move |this, _cx| {
                        let mut st = this.state.borrow_mut();
                        st.create_measurement_track(kind);
                        st.commit_edit();
                        st.select_tool(match kind {
                            MeasurementKind::Tape => Tool::Tape,
                            MeasurementKind::Protractor => Tool::Protractor,
                            MeasurementKind::Circle => Tool::Circle,
                        });
                    },
                ));
            }
            items.push(menu_item(
                tr("menu.item.delete_track"),
                has_track && !tracking,
                cx,
                |this, _cx| {
                    this.state.borrow_mut().delete_active_track();
                },
            ));
            items.push(menu_item(
                tr("menu.item.clear_track"),
                has_track && !tracking,
                cx,
                |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    if let Some(track) = st.active_track_mut() {
                        track.clear_points();
                        st.commit_edit();
                    }
                },
            ));
            if has_track {
                items.push(menu_separator());
                items.push(menu_item_dynamic(
                    t!("menu.item.current_track", name = track_name).to_string(),
                    20,
                    false,
                    cx,
                    |_this, _cx| {},
                ));
            }
            items.push(menu_item_dynamic(
                check(tr("menu.item.visible"), visible),
                21,
                has_track,
                cx,
                move |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    if let Some(track) = st.active_track_mut() {
                        track.visible = !visible;
                        st.commit_edit();
                    }
                },
            ));
            items.push(menu_item_dynamic(
                check(tr("menu.item.locked"), locked),
                22,
                has_track,
                cx,
                move |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    if let Some(track) = st.active_track_mut() {
                        track.locked = !locked;
                        st.commit_edit();
                    }
                },
            ));
            items.push(menu_item_dynamic(
                t!("menu.item.footprint", label = footprint.label()).to_string(),
                23,
                has_track,
                cx,
                move |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    if let Some(track) = st.active_track_mut() {
                        track.footprint = footprint.next();
                        st.commit_edit();
                    }
                },
            ));
            items.push(menu_item_dynamic(
                t!("menu.item.step", n = step_size, sign = "-").to_string(),
                24,
                has_track,
                cx,
                move |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    if let Some(track) = st.active_track_mut() {
                        track.step_size = track.step_size.saturating_sub(1).max(1);
                        st.commit_edit();
                    }
                },
            ));
            items.push(menu_item_dynamic(
                t!("menu.item.step", n = step_size, sign = "+").to_string(),
                25,
                has_track,
                cx,
                move |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    if let Some(track) = st.active_track_mut() {
                        track.step_size = (track.step_size + 1).min(1000);
                        st.commit_edit();
                    }
                },
            ));
            items.push(menu_item_dynamic(
                check(
                    t!("menu.item.autostep", n = step_size).to_string(),
                    auto_advance,
                ),
                26,
                has_track,
                cx,
                move |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    if let Some(track) = st.active_track_mut() {
                        track.auto_advance = !auto_advance;
                        st.commit_edit();
                    }
                },
            ));
            items.push(menu_item(
                tr("menu.item.delete_point"),
                has_track && !locked && !tracking,
                cx,
                move |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    let (frame, active) = (st.frame, st.active);
                    st.apply_edit_command(EditCommand::DeletePoint {
                        track: active,
                        frame,
                    });
                },
            ));
            items.push(menu_separator());
            #[cfg(feature = "vision")]
            items.push(menu_item(
                tr("menu.item.autodetect"),
                has_video && !tracking && !auto_detect_running,
                cx,
                |this, cx| {
                    this.start_motion_detection(cx);
                },
            ));
            #[cfg(feature = "vision")]
            items.push(menu_item(
                tr("menu.item.aruco"),
                has_video && !tracking && !auto_detect_running,
                cx,
                |this, cx| {
                    this.start_rigid_tracking(cx);
                },
            ));
            #[cfg(feature = "vision")]
            items.push(menu_item(
                tr("menu.item.markerless"),
                has_video && !tracking && !auto_detect_running,
                cx,
                |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    st.rigid_roi_mode = !st.rigid_roi_mode;
                    st.motion_roi_mode = false;
                    st.tool = Tool::Move;
                    st.status = st
                        .rigid_roi_mode
                        .then(|| tr("menu.item.markerless_hint"));
                },
            ));
            #[cfg(feature = "vision")]
            items.push(menu_item(
                tr("menu.item.motion_contour"),
                has_video && !tracking && !auto_detect_running,
                cx,
                |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    st.motion_roi_mode = !st.motion_roi_mode;
                    st.rigid_roi_mode = false;
                    st.tool = Tool::Move;
                    st.status = st
                        .motion_roi_mode
                        .then(|| tr("menu.item.motion_hint"));
                },
            ));
            #[cfg(feature = "vision")]
            items.push(menu_item_dynamic(
                check(tr("menu.item.pure_mask"), motion_pure_mask),
                28,
                has_video && !tracking && !auto_detect_running,
                cx,
                move |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    st.motion_pure_mask = !motion_pure_mask;
                    st.status = Some(if st.motion_pure_mask {
                        tr("menu.status.motion_mask_on")
                    } else {
                        tr("menu.status.motion_mask_off")
                    });
                },
            ));
            items.push(menu_item(
                "AutoTracker…",
                can_start || tracking,
                cx,
                |this, cx| {
                    this.open_tracker_window(cx);
                },
            ));
            items.push(menu_item(
                tr("menu.item.stop_tracking"),
                tracking,
                cx,
                |this, _cx| {
                    this.state.borrow_mut().stop_tracking();
                },
            ));
            if !has_seed && has_track {
                items.push(menu_item_dynamic(
                    tr("menu.status.need_seed"),
                    27,
                    false,
                    cx,
                    |_this, _cx| {},
                ));
            }
        }
        MenuId::Coords => {
            let (fixed_cal, locked, fixed_origin, fixed_angle) = {
                let st = state.borrow();
                (
                    st.calibration.fixed_coordinates,
                    st.coords.locked,
                    st.coords.fixed_origin,
                    st.coords.fixed_angle,
                )
            };
            items.push(menu_item(
                tr("menu.item.new_stick"),
                has_docs && !locked,
                cx,
                |this, _cx| {
                    this.state.borrow_mut().add_calibration_stick();
                },
            ));
            items.push(menu_item_dynamic(
                check(tr("menu.item.fixed_cal"), fixed_cal),
                0,
                has_docs && !locked,
                cx,
                move |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    st.calibration.fixed_coordinates = !fixed_cal;
                    st.apply_calibration();
                    st.commit_edit();
                },
            ));
            items.push(menu_separator());
            items.push(menu_item_dynamic(
                check(tr("menu.item.lock_coords"), locked),
                1,
                has_docs,
                cx,
                move |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    st.coords.locked = !locked;
                    st.commit_edit();
                },
            ));
            items.push(menu_item_dynamic(
                check(tr("menu.item.fixed_origin"), fixed_origin),
                2,
                has_docs && !locked,
                cx,
                move |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    let frame = st.frame;
                    st.coords.set_fixed_origin(!fixed_origin, frame);
                    st.commit_edit();
                },
            ));
            items.push(menu_item_dynamic(
                check(tr("menu.item.fixed_angle"), fixed_angle),
                3,
                has_docs && !locked,
                cx,
                move |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    let frame = st.frame;
                    st.coords.set_fixed_angle(!fixed_angle, frame);
                    st.commit_edit();
                },
            ));
            items.push(menu_separator());
            items.push(menu_item_dynamic(
                tr("menu.item.align_stick"),
                5,
                has_docs && !locked,
                cx,
                |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    st.apply_calibration();
                    st.commit_edit();
                },
            ));
            items.push(menu_item_dynamic(
                tr("menu.item.unify_frame"),
                6,
                has_docs && !locked,
                cx,
                |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    let frame = st.frame;
                    st.coords.set_all_values_to_frame(frame);
                    st.commit_edit();
                },
            ));
            items.push(menu_item_dynamic(
                tr("menu.item.reset_coords"),
                7,
                has_docs && !locked,
                cx,
                |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    st.coords.reset();
                    st.commit_edit();
                },
            ));
            items.push(menu_item_dynamic(
                tr("menu.item.reset_cal"),
                8,
                has_docs && !locked,
                cx,
                |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    st.apply_edit_command(EditCommand::ResetCalibration);
                },
            ));
        }
        MenuId::Help => {
            items.push(menu_item(tr("menu.item.help_guide"), true, cx, |this, _cx| {
                this.state.borrow_mut().modal = Some(Modal::Operations);
            }));
            items.push(menu_separator());
            items.push(menu_item(tr("menu.item.about"), true, cx, |this, _cx| {
                this.state.borrow_mut().modal = Some(Modal::About);
            }));
        }
    }

    crate::overlay::overlay_surface()
        .absolute()
        .top(TITLE_BAR_HEIGHT)
        .left(px(x.clamp(4.0, 340.0)))
        .w(px(250.))
        .flex()
        .flex_col()
        .py_1()
        .bg(c(0x152238))
        .border_1()
        .border_color(c(0x2a3b55))
        .rounded_sm()
        .children(items)
        .into_any_element()
}
