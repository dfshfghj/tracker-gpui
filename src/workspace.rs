use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::i18n::tr;
use crate::*;

pub(crate) fn scan_videos(dir: &std::path::Path, depth: usize) -> Vec<FileNode> {
    if depth == 0 {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if matches!(
            name.as_str(),
            "target" | "node_modules" | "vendor" | "third_party"
        ) {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            let children = scan_videos(&path, depth - 1);
            if !children.is_empty() {
                dirs.push(FileNode {
                    name,
                    path,
                    is_dir: true,
                    children,
                });
            }
        } else if file_type.is_file() {
            let is_video = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| VIDEO_EXTS.contains(&ext.to_ascii_lowercase().as_str()))
                .unwrap_or(false);
            if is_video {
                files.push(FileNode {
                    name,
                    path,
                    is_dir: false,
                    children: Vec::new(),
                });
            }
        }
    }
    dirs.sort_by_key(|node| node.name.to_lowercase());
    files.sort_by_key(|node| node.name.to_lowercase());
    dirs.extend(files);
    dirs
}

// ---------------------------------------------------------------- drawing

pub(crate) fn tree_rows(
    state: Rc<RefCell<State>>,
    nodes: &[FileNode],
    depth: usize,
    counter: &mut usize,
    cx: &mut Context<Probe>,
) -> Vec<AnyElement> {
    let mut rows = Vec::new();
    for node in nodes {
        let row_id = *counter;
        *counter += 1;
        if node.is_dir {
            let expanded = state.borrow().expanded.contains(&node.path);
            let toggle_state = state.clone();
            let path = node.path.clone();
            rows.push(
                div()
                    .id(("dir", row_id))
                    .flex()
                    .items_center()
                    .gap_1()
                    .pl(px(4. + depth as f32 * 14.))
                    .h(px(24.))
                    .rounded_sm()
                    .text_xs()
                    .text_color(c(0x9fb2cc))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |_this, _ev, _window, cx| {
                            let mut st = toggle_state.borrow_mut();
                            if !st.expanded.remove(&path) {
                                st.expanded.insert(path.clone());
                            }
                            drop(st);
                            cx.notify();
                        }),
                    )
                    .child(div().w(px(12.)).text_color(c(0x5f6f88)).child(if expanded {
                        "▾"
                    } else {
                        "▸"
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .child(node.name.clone()),
                    )
                    .into_any_element(),
            );
            if expanded {
                rows.extend(tree_rows(
                    state.clone(),
                    &node.children,
                    depth + 1,
                    counter,
                    cx,
                ));
            }
        } else {
            let open_state = state.clone();
            let path = node.path.clone();
            let (is_open, is_active) = {
                let st = state.borrow();
                (
                    st.docs.iter().any(|doc| doc.path == node.path),
                    st.path == node.path && !st.is_placeholder(),
                )
            };
            rows.push(
                div()
                    .id(("video", row_id))
                    .flex()
                    .items_center()
                    .gap_1()
                    .pl(px(20. + depth as f32 * 14.))
                    .h(px(24.))
                    .rounded_sm()
                    .bg(if is_active { c(0x16233a) } else { c(0x0d1420) })
                    .text_xs()
                    .text_color(if is_active {
                        c(0xdbe6f5)
                    } else if is_open {
                        c(0xb9cbe4)
                    } else {
                        c(0x9fb2cc)
                    })
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |_this, _ev, _window, cx| {
                            open_video(open_state.clone(), path.clone(), cx);
                        }),
                    )
                    .child(div().w(px(12.)).text_color(c(0x4dabf7)).child("▶"))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .child(node.name.clone()),
                    )
                    .into_any_element(),
            );
        }
    }
    rows
}

pub(crate) fn clamp_tab_offset(offset: f32, delta: f32, max_offset: f32) -> f32 {
    let max_scroll = if max_offset.is_finite() {
        max_offset.max(0.0)
    } else {
        0.0
    };
    let target = offset - delta;
    if target.is_nan() {
        return 0.0;
    }
    target.clamp(-max_scroll, 0.0)
}

pub(crate) fn tab_bar(state: Rc<RefCell<State>>, cx: &mut Context<Probe>) -> impl IntoElement {
    let (tabs, active) = {
        let st = state.borrow();
        (
            st.docs
                .iter()
                .enumerate()
                .filter(|(_, doc)| !doc.is_placeholder())
                .map(|(index, doc)| {
                    (
                        index,
                        doc.title.clone(),
                        doc.is_dirty(),
                        doc.loading
                            .as_ref()
                            .map(|loading| loading.progress.load(Ordering::Relaxed)),
                    )
                })
                .collect::<Vec<_>>(),
            st.active_doc,
        )
    };

    let bar = div()
        .h(px(34.))
        .w_full()
        .flex_none()
        .flex()
        .items_center()
        .px_2()
        .bg(c(0x0a1018))
        .border_b_1()
        .border_color(c(0x1d2a3d))
        .overflow_hidden();

    let mut tabs_inner = div().flex().items_center().gap_1().flex_none().h_full();
    for (index, title, dirty, loading) in tabs {
        let title = if dirty { format!("{title} *") } else { title };
        let selected = index == active;
        let activate_state = state.clone();
        let close_state = state.clone();
        tabs_inner = tabs_inner.child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap_1()
                .h(px(28.))
                .max_w(px(230.))
                .px_2()
                .rounded_sm()
                .bg(if selected { c(0x16233a) } else { c(0x0d1420) })
                .border_t_1()
                .border_color(if selected { c(0x4dabf7) } else { c(0x1d2a3d) })
                .text_xs()
                .text_color(if selected { c(0xdbe6f5) } else { c(0x8fa4c0) })
                .child(
                    div()
                        .id(("tab", index))
                        .max_w(px(160.))
                        .truncate()
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |_this, _ev, _window, cx| {
                                activate_state.borrow_mut().activate_doc(index);
                                cx.notify();
                            }),
                        )
                        .child(title),
                )
                .when_some(loading, |this, percent| {
                    this.child(div().text_color(c(0xffd43b)).child(format!("{percent}%")))
                })
                .child(
                    div()
                        .id(("close-tab", index))
                        .px_1()
                        .rounded_sm()
                        .cursor_pointer()
                        .text_color(c(0x6f819c))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |_this, _ev, _window, cx| {
                                request_close_doc(close_state.clone(), index, cx);
                            }),
                        )
                        .child("×"),
                ),
        );
    }

    let scroll_handle = state.borrow().tab_scroll.clone();
    let tabs_area = div()
        .id("tab-scroll-area")
        .relative()
        .flex_1()
        .min_w(px(0.))
        .h_full()
        .flex()
        .items_center()
        .overflow_x_scroll()
        .track_scroll(&scroll_handle)
        .on_scroll_wheel(
            cx.listener(move |this, ev: &ScrollWheelEvent, _window, cx| {
                let delta = match ev.delta {
                    ScrollDelta::Pixels(p) => {
                        if f32::from(p.x).abs() > f32::from(p.y).abs() {
                            f32::from(p.x)
                        } else {
                            f32::from(p.y)
                        }
                    }
                    ScrollDelta::Lines(p) => {
                        if p.x.abs() > p.y.abs() {
                            p.x * 20.0
                        } else {
                            p.y * 20.0
                        }
                    }
                };
                if delta.abs() < 0.1 {
                    return;
                }
                let handle = this.state.borrow().tab_scroll.clone();
                let offset = handle.offset();
                let new_x =
                    clamp_tab_offset(f32::from(offset.x), delta, f32::from(handle.max_offset().x));
                handle.set_offset(point(px(new_x), offset.y));
                cx.notify();
            }),
        )
        .child(tabs_inner);
    bar.child(tabs_area)
}

pub(crate) fn collapsed_rail(left_side: bool, cx: &mut Context<Probe>) -> AnyElement {
    let (id, icon, tooltip) = if left_side {
        ("rail-left", ICON_FWD1, tr("rail.expand_workspace"))
    } else {
        ("rail-right", ICON_BACK1, tr("rail.expand_inspector"))
    };
    div()
        .w(px(24.))
        .flex_none()
        .h_full()
        .flex()
        .flex_col()
        .items_center()
        .pt_2()
        .bg(c(0x0d1420))
        .when(left_side, |this| this.border_r_1())
        .when(!left_side, |this| this.border_l_1())
        .border_color(c(0x1d2a3d))
        .child(icon_button(
            id,
            icon,
            tooltip,
            20.0,
            12.0,
            false,
            true,
            cx,
            move |this, _cx| {
                let mut st = this.state.borrow_mut();
                if left_side {
                    st.left_open = true;
                } else {
                    st.right_open = true;
                }
            },
        ))
        .into_any_element()
}

pub(crate) fn video_controls(
    state: Rc<RefCell<State>>,
    cx: &mut Context<Probe>,
) -> impl IntoElement {
    let prepaint_state = state.clone();
    let paint_state = state.clone();

    let progress_bar = div()
        .id("video-progress")
        .flex_1()
        .h(px(18.))
        .relative()
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, ev: &MouseDownEvent, _window, cx| {
                let mut st = this.state.borrow_mut();
                if st.loading.is_some() {
                    return;
                }
                let (left, width) = (st.progress_rect.0, st.progress_rect.2);
                if width > 1.0 {
                    st.scrubbing = true;
                    st.seek_fraction((f32::from(ev.position.x) - left) / width);
                    drop(st);
                    cx.notify();
                }
            }),
        )
        .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _window, cx| {
            let mut st = this.state.borrow_mut();
            if !st.scrubbing || st.loading.is_some() {
                return;
            }
            let (left, width) = (st.progress_rect.0, st.progress_rect.2);
            if width > 1.0 {
                st.seek_fraction((f32::from(ev.position.x) - left) / width);
                drop(st);
                cx.notify();
            }
        }))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _ev: &MouseUpEvent, _window, cx| {
                this.state.borrow_mut().scrubbing = false;
                cx.notify();
            }),
        )
        .child(
            canvas(
                move |bounds, _window, _cx| {
                    let mut st = prepaint_state.borrow_mut();
                    st.progress_rect = (
                        f32::from(bounds.origin.x),
                        f32::from(bounds.origin.y),
                        f32::from(bounds.size.width),
                        f32::from(bounds.size.height),
                    );
                },
                move |bounds, _prepaint, window, _cx| {
                    let (fraction, loading) = {
                        let st = paint_state.borrow();
                        (st.progress_fraction().clamp(0.0, 1.0), st.loading.is_some())
                    };
                    let bar_height = px(4.);
                    let y = bounds.origin.y + (bounds.size.height - bar_height) / 2.0;
                    let track = Bounds {
                        origin: point(bounds.origin.x, y),
                        size: size(bounds.size.width, bar_height),
                    };
                    window.paint_quad(quad(
                        track,
                        px(2.),
                        c(0x1d2a3d),
                        px(0.),
                        transparent_black(),
                        Default::default(),
                    ));
                    if fraction <= 0.0 {
                        return;
                    }
                    let fill = Bounds {
                        origin: track.origin,
                        size: size(bounds.size.width * fraction, bar_height),
                    };
                    window.paint_quad(quad(
                        fill,
                        px(2.),
                        if loading { c(0xffd43b) } else { c(0x4dabf7) },
                        px(0.),
                        transparent_black(),
                        Default::default(),
                    ));
                    let head_x = bounds.origin.x + bounds.size.width * fraction;
                    let head = Bounds {
                        origin: point(
                            head_x - px(4.),
                            bounds.origin.y + (bounds.size.height - px(10.)) / 2.0,
                        ),
                        size: size(px(8.), px(10.)),
                    };
                    window.paint_quad(quad(
                        head,
                        px(4.),
                        c(0xdbe6f5),
                        px(0.),
                        transparent_black(),
                        Default::default(),
                    ));
                },
            )
            .size_full(),
        );

    let (left_label, right_label) = {
        let st = state.borrow();
        if let Some(loading) = &st.loading {
            (
                tr("tree.indexing"),
                format!("{}%", loading.progress.load(Ordering::Relaxed)),
            )
        } else {
            (format_time(st.current_time()), format_time(st.total_time()))
        }
    };

    let progress_row = div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .w(px(48.))
                .text_xs()
                .text_color(c(0x9fb2cc))
                .child(left_label),
        )
        .child(progress_bar)
        .child(
            div()
                .w(px(48.))
                .flex()
                .justify_end()
                .text_xs()
                .text_color(c(0x7d8ca6))
                .child(right_label),
        );

    let (playing, frame, last, time) = {
        let st = state.borrow();
        (
            st.playing,
            st.frame,
            st.frame_count.saturating_sub(1),
            st.current_time(),
        )
    };
    let buttons = [
        (Cmd::First, ICON_FIRST, tr("transport.first")),
        (Cmd::Back10, ICON_BACK10, tr("transport.back10")),
        (Cmd::Back1, ICON_BACK1, tr("transport.back1")),
        (
            Cmd::Play,
            if playing { ICON_PAUSE } else { ICON_PLAY },
            if playing { tr("transport.pause") } else { tr("transport.play") },
        ),
        (Cmd::Fwd1, ICON_FWD1, tr("transport.fwd1")),
        (Cmd::Fwd10, ICON_FWD10, tr("transport.fwd10")),
        (Cmd::Last, ICON_LAST, tr("transport.last")),
    ];
    let button_row = div()
        .flex()
        .items_center()
        .gap_1()
        .children(buttons.into_iter().map(|(cmd, icon, tooltip)| {
            let id = match cmd {
                Cmd::First => "cmd-first",
                Cmd::Back10 => "cmd-back10",
                Cmd::Back1 => "cmd-back1",
                Cmd::Play => "cmd-play",
                Cmd::Fwd1 => "cmd-fwd1",
                Cmd::Fwd10 => "cmd-fwd10",
                Cmd::Last => "cmd-last",
            };
            let active = matches!(cmd, Cmd::Play) && playing;
            icon_button(
                id,
                icon,
                tooltip,
                30.0,
                15.0,
                active,
                true,
                cx,
                move |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    match cmd {
                        Cmd::First => {
                            st.pause();
                            st.set_frame(0);
                        }
                        Cmd::Back10 => {
                            st.pause();
                            let next = st.frame.saturating_sub(10);
                            st.set_frame(next);
                        }
                        Cmd::Back1 => {
                            st.pause();
                            let next = st.frame.saturating_sub(1);
                            st.set_frame(next);
                        }
                        Cmd::Play => {
                            if st.playing {
                                st.pause();
                            } else {
                                st.play();
                            }
                        }
                        Cmd::Fwd1 => {
                            st.pause();
                            let next = st.frame + 1;
                            st.set_frame(next);
                        }
                        Cmd::Fwd10 => {
                            st.pause();
                            let next = st.frame + 10;
                            st.set_frame(next);
                        }
                        Cmd::Last => {
                            st.pause();
                            let next = st.frame_count.saturating_sub(1);
                            st.set_frame(next);
                        }
                    }
                },
            )
        }))
        .child(div().flex_1())
        .child(
            div()
                .text_xs()
                .text_color(c(0x7d8ca6))
                .child(format!("frame {frame} / {last}   {}", format_time(time))),
        );

    div()
        .flex_none()
        .w_full()
        .flex()
        .flex_col()
        .gap_2()
        .px_3()
        .py_2()
        .bg(c(0x0a1018))
        .border_t_1()
        .border_color(c(0x1d2a3d))
        .child(progress_row)
        .child(button_row)
}
