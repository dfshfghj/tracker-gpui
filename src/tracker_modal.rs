use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gpui_component::scroll::ScrollableElement;
use gpui_kit::*;

use crate::i18n::tr;
use rust_i18n::t;
use crate::*;

struct TrackerDialogData {
    inputs: [String; 6],
    field: TrackerField,
    error: Option<String>,
    settings: AutoTrackSettings,
    fast: bool,
    track_name: String,
    frame: usize,
    frame_count: usize,
    keyframes: usize,
    running: bool,
    paused: bool,
    points: usize,
    confidence: f32,
    status: String,
}

fn tracker_choice(
    id: usize,
    label: impl Into<SharedString>,
    active: bool,
    cx: &mut Context<Probe>,
    action: impl Fn(&mut Probe, &mut Context<Probe>) + 'static,
) -> AnyElement {
    let label = label.into();
    div()
        .id(("tracker-choice", id))
        .px_2()
        .py_0p5()
        .rounded_sm()
        .text_xs()
        .cursor_pointer()
        .bg(if active { c(0x1b2b45) } else { c(0x111a29) })
        .text_color(if active { c(0x8ecbff) } else { c(0x9fb2cc) })
        .hover(|style| style.bg(c(0x152238)))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _ev, _window, cx| {
                action(this, cx);
                cx.notify();
            }),
        )
        .child(label)
        .into_any_element()
}

fn tracker_num_field(
    label: impl Into<SharedString>,
    value: String,
    active: bool,
    field: TrackerField,
    state: Rc<RefCell<State>>,
    cx: &mut Context<Probe>,
) -> AnyElement {
    let label = label.into();
    div()
        .flex()
        .items_center()
        .gap_1()
        .child(div().text_xs().text_color(c(0x7d8ca6)).child(label))
        .child(
            div()
                .id(("tracker-num", field.index()))
                .w(px(58.))
                .px_1()
                .py_0p5()
                .rounded_sm()
                .bg(if active { c(0x16233a) } else { c(0x111a29) })
                .border_1()
                .border_color(if active { c(0x4dabf7) } else { c(0x1d2a3d) })
                .text_xs()
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |_this, _ev, _window, cx| {
                        state.borrow_mut().tracker_field = field;
                        cx.notify();
                    }),
                )
                .child(format!("{value}{}", if active { "▏" } else { "" })),
        )
        .into_any_element()
}

fn tracker_thumb(label: impl Into<SharedString>, image: Option<Arc<RenderImage>>) -> AnyElement {
    let label = label.into();
    let mut panel = div()
        .flex()
        .flex_col()
        .items_center()
        .gap_1()
        .child(div().text_xs().text_color(c(0x7d8ca6)).child(label));
    if let Some(image) = image {
        panel = panel.child(
            img(image)
                .w(px(54.))
                .h(px(54.))
                .rounded_sm()
                .border_1()
                .border_color(c(0x2a3b55)),
        );
    } else {
        panel = panel.child(
            div()
                .w(px(54.))
                .h(px(54.))
                .rounded_sm()
                .bg(c(0x111a29))
                .border_1()
                .border_color(c(0x2a3b55))
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(c(0x4a5a73))
                .child("—"),
        );
    }
    panel.into_any_element()
}

fn dialog_header(data: &TrackerDialogData) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .flex_1()
                .truncate()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .child(t!("tracker_ui.title", name = data.track_name).to_string()),
        )
        .child(div().text_xs().text_color(c(0x7d8ca6)).child(t!(
            "tracker_ui.header",
            frame = data.frame,
            count = data.frame_count,
            keyframes = data.keyframes
        )
        .to_string()))
        .into_any_element()
}

fn dialog_actions(
    data: &TrackerDialogData,
    state: Rc<RefCell<State>>,
    cx: &mut Context<Probe>,
) -> AnyElement {
    let search_label = if data.running {
        if data.paused {
            tr("tracker_ui.resume")
        } else {
            tr("tracker_ui.pause")
        }
    } else {
        tr("tracker_ui.search")
    };
    let search_state = state.clone();
    let back_state = state.clone();
    let next_state = state.clone();
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(modal_button(search_label, true, cx, move |_this, _cx| {
            search_state.borrow_mut().tracker_search_action();
        }))
        .child(modal_button(tr("tracker_ui.back"), false, cx, move |_this, _cx| {
            back_state.borrow_mut().tracker_step_back();
        }))
        .child(modal_button(
            tr("tracker_ui.search_next"),
            false,
            cx,
            move |_this, _cx| {
                let mut st = next_state.borrow_mut();
                if st.tracking.is_some() {
                    st.pause_tracking();
                }
                if st.apply_tracker_dialog() {
                    st.single_step_tracking_ui();
                }
            },
        ))
        .child(div().flex_1())
        .child(div().text_xs().text_color(c(0x5f6f88)).child(t!(
            "tracker_ui.policy",
            search = data.settings.search_policy.label(),
            stop = data.settings.stop_policy.label()
        )
        .to_string()))
        .into_any_element()
}

fn dialog_preview(
    data: &TrackerDialogData,
    template: Option<Arc<RenderImage>>,
    matched: Option<Arc<RenderImage>>,
    cx: &mut Context<Probe>,
) -> AnyElement {
    let score = if data.confidence.is_finite() {
        format!("{:.1}", data.confidence)
    } else {
        "—".to_string()
    };
    let info_color = if data.paused {
        c(0xffd43b)
    } else if data.running {
        c(0x51cf66)
    } else {
        c(0x5f6f88)
    };
    let info = if data.running {
        t!(
            "tracker_ui.info",
            state = if data.paused {
                tr("tracker_ui.paused")
            } else {
                tr("tracker_ui.searching")
            },
            frame = data.frame,
            points = data.points,
            score = score
        )
        .to_string()
    } else {
        data.status.clone()
    };
    let _ = cx;
    div()
        .flex()
        .items_center()
        .gap_3()
        .p_2()
        .rounded_sm()
        .bg(c(0x0a1018))
        .child(
            div()
                .text_xs()
                .text_color(c(0x8fa3bf))
                .child(t!("tracker_ui.frame", frame = data.frame).to_string()),
        )
        .child(tracker_thumb(tr("tracker_ui.template"), template))
        .child(div().text_xs().text_color(c(0x5f6f88)).child("="))
        .child(tracker_thumb(tr("tracker_ui.matched"), matched))
        .child(div().flex_1())
        .child(
            div()
                .max_w(px(300.))
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_xs().text_color(info_color).child(info))
                .child(div().text_xs().text_color(info_color).child(t!(
                    "tracker_ui.match_hint",
                    frame = data.frame,
                    score = score
                )
                .to_string())),
        )
        .into_any_element()
}

fn dialog_fields(
    data: &TrackerDialogData,
    state: Rc<RefCell<State>>,
    cx: &mut Context<Probe>,
) -> AnyElement {
    let settings = &data.settings;
    let search_policy = settings.search_policy;
    let stop_policy = settings.stop_policy;
    let shape = settings.shape;
    let template_row = div()
        .flex()
        .items_center()
        .gap_3()
        .child(
            div()
                .w(px(48.))
                .text_xs()
                .text_color(c(0x8fa3bf))
                .child(tr("tracker_ui.template_label")),
        )
        .child(tracker_num_field(
            tr("tracker_ui.evolve"),
            data.inputs[3].clone(),
            data.field == TrackerField::Evolve,
            TrackerField::Evolve,
            state.clone(),
            cx,
        ))
        .child(tracker_num_field(
            "Tether %",
            data.inputs[4].clone(),
            data.field == TrackerField::Tether,
            TrackerField::Tether,
            state.clone(),
            cx,
        ))
        .child(tracker_num_field(
            tr("tracker_ui.auto_mark"),
            data.inputs[5].clone(),
            data.field == TrackerField::AutoMark,
            TrackerField::AutoMark,
            state.clone(),
            cx,
        ));
    let shape_row = div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .w(px(48.))
                .text_xs()
                .text_color(c(0x8fa3bf))
                .child(tr("tracker_ui.shape")),
        )
        .child(tracker_choice(
            410,
            tr("tracker_ui.ellipse"),
            shape == MaskShape::Ellipse,
            cx,
            |this, _cx| {
                let mut st = this.state.borrow_mut();
                st.tracker_settings.shape = MaskShape::Ellipse;
                st.sync_tracker_overlay();
            },
        ))
        .child(tracker_choice(
            411,
            tr("tracker_ui.rect"),
            shape == MaskShape::Rectangle,
            cx,
            |this, _cx| {
                let mut st = this.state.borrow_mut();
                st.tracker_settings.shape = MaskShape::Rectangle;
                st.sync_tracker_overlay();
            },
        ))
        .child(div().w(px(12.)).h(px(1.)).bg(c(0x1d2a3d)))
        .child(tracker_num_field(
            tr("tracker_ui.width"),
            data.inputs[0].clone(),
            data.field == TrackerField::MaskWidth,
            TrackerField::MaskWidth,
            state.clone(),
            cx,
        ))
        .child(tracker_num_field(
            tr("tracker_ui.height"),
            data.inputs[1].clone(),
            data.field == TrackerField::MaskHeight,
            TrackerField::MaskHeight,
            state.clone(),
            cx,
        ))
        .child(tracker_num_field(
            tr("tracker_ui.search"),
            data.inputs[2].clone(),
            data.field == TrackerField::Search,
            TrackerField::Search,
            state.clone(),
            cx,
        ));
    let search_row = div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .w(px(48.))
                .text_xs()
                .text_color(c(0x8fa3bf))
                .child(tr("tracker_ui.search_label")),
        )
        .child(check_item(400, "Fast", data.fast, true, cx, |this, _cx| {
            let mut st = this.state.borrow_mut();
            st.tracker_fast = !st.tracker_fast;
        }))
        .child(div().w(px(12.)).h(px(1.)).bg(c(0x1d2a3d)))
        .child(tracker_choice(
            420,
            "Look-Ahead",
            search_policy == SearchPolicy::LookAhead,
            cx,
            |this, _cx| {
                let mut st = this.state.borrow_mut();
                st.tracker_settings.search_policy = SearchPolicy::LookAhead;
                st.sync_tracker_overlay();
            },
        ))
        .child(tracker_choice(
            421,
            "Follow",
            search_policy == SearchPolicy::Follow,
            cx,
            |this, _cx| {
                let mut st = this.state.borrow_mut();
                st.tracker_settings.search_policy = SearchPolicy::Follow;
                st.sync_tracker_overlay();
            },
        ))
        .child(tracker_choice(
            422,
            "Fixed",
            search_policy == SearchPolicy::Fixed,
            cx,
            |this, _cx| {
                let mut st = this.state.borrow_mut();
                st.tracker_settings.search_policy = SearchPolicy::Fixed;
                st.sync_tracker_overlay();
            },
        ))
        .child(div().w(px(12.)).h(px(1.)).bg(c(0x1d2a3d)))
        .child(check_item(
            401,
            tr("tracker_ui.only_x"),
            settings.one_d,
            true,
            cx,
            |this, _cx| {
                let mut st = this.state.borrow_mut();
                st.tracker_settings.one_d = !st.tracker_settings.one_d;
                st.sync_tracker_overlay();
            },
        ));
    let stop_row = div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .w(px(48.))
                .text_xs()
                .text_color(c(0x8fa3bf))
                .child("Stop:"),
        )
        .child(tracker_choice(
            430,
            "Poor/No Match",
            stop_policy == StopPolicy::StopOnPoor,
            cx,
            |this, _cx| {
                this.state.borrow_mut().tracker_settings.stop_policy = StopPolicy::StopOnPoor;
            },
        ))
        .child(tracker_choice(
            431,
            "No Match",
            stop_policy == StopPolicy::StopOnNoMatch,
            cx,
            |this, _cx| {
                this.state.borrow_mut().tracker_settings.stop_policy = StopPolicy::StopOnNoMatch;
            },
        ))
        .child(tracker_choice(
            432,
            "Never",
            stop_policy == StopPolicy::Never,
            cx,
            |this, _cx| {
                this.state.borrow_mut().tracker_settings.stop_policy = StopPolicy::Never;
            },
        ));
    let track_state = state.clone();
    let track_label = data.track_name.clone();
    let target_row = div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .w(px(48.))
                .text_xs()
                .text_color(c(0x8fa3bf))
                .child(tr("tracker_ui.target")),
        )
        .child(div().text_xs().text_color(c(0x7d8ca6)).child(tr("tracker_ui.trail")))
        .child(
            div()
                .id("tracker-target-track")
                .max_w(px(160.))
                .truncate()
                .px_2()
                .py_0p5()
                .rounded_sm()
                .bg(c(0x1b2b45))
                .text_xs()
                .text_color(c(0x8ecbff))
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |_this, _ev, _window, cx| {
                        track_state.borrow_mut().cycle_tracker_track();
                        cx.notify();
                    }),
                )
                .child(track_label),
        )
        .child(div().text_xs().text_color(c(0x7d8ca6)).child(tr("tracker_ui.points")))
        .child(
            div()
                .px_2()
                .py_0p5()
                .rounded_sm()
                .bg(c(0x111a29))
                .text_xs()
                .text_color(c(0x9fb2cc))
                .child(tr("tracker_ui.position")),
        );
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(template_row)
        .child(shape_row)
        .child(search_row)
        .child(stop_row)
        .child(target_row)
        .into_any_element()
}

fn dialog_footer(
    data: &TrackerDialogData,
    state: Rc<RefCell<State>>,
    cx: &mut Context<Probe>,
) -> AnyElement {
    let help_state = state.clone();
    let key_state = state.clone();
    let delete_state = state.clone();
    let copy_state = state.clone();
    let close_state = state.clone();
    let mut footer = div().flex().flex_col().gap_2();
    if let Some(error) = &data.error {
        footer = footer.child(div().text_xs().text_color(c(0xff6b6b)).child(error.clone()));
    }
    footer
        .child(
            div()
                .text_xs()
                .text_color(c(0x5f6f88))
                .child(tr("tracker_ui.hint")),
        )
        .child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(modal_button(tr("tracker_ui.help"), false, cx, move |_this, _cx| {
                    let mut st = help_state.borrow_mut();
                    st.close_tracker_dialog();
                    st.modal = Some(Modal::Operations);
                }))
                .child(modal_button(
                    tr("tracker_ui.show_keyframe"),
                    false,
                    cx,
                    move |_this, _cx| {
                        key_state.borrow_mut().tracker_show_keyframe();
                    },
                ))
                .child(modal_button(tr("tracker_ui.delete"), false, cx, move |_this, _cx| {
                    delete_state.borrow_mut().tracker_delete_current();
                }))
                .child(modal_button(
                    "Copy Match Data",
                    false,
                    cx,
                    move |_this, cx| {
                        let text = copy_state.borrow().tracker_match_csv();
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                        copy_state.borrow_mut().status = Some(tr("tracker_ui.copied"));
                    },
                ))
                .child(modal_button(tr("tracker_ui.close"), false, cx, move |_this, _cx| {
                    close_state.borrow_mut().close_tracker_dialog();
                })),
        )
        .into_any_element()
}

pub(crate) fn tracker_settings_modal_view(
    state: Rc<RefCell<State>>,
    cx: &mut Context<Probe>,
) -> AnyElement {
    let (template, matched) = state.borrow_mut().tracker_thumbnails();
    let data = {
        let st = state.borrow();
        TrackerDialogData {
            inputs: st.tracker_inputs.clone(),
            field: st.tracker_field,
            error: st.tracker_error.clone(),
            settings: st.tracker_settings.clone(),
            fast: st.tracker_fast,
            track_name: st
                .active_track()
                .map(|track| track.name.clone())
                .unwrap_or_else(|| tr("tracker_ui.no_track")),
            frame: st.frame,
            frame_count: st.frame_count.saturating_sub(1),
            keyframes: st
                .active_track()
                .map(|track| track.keyframes().count())
                .unwrap_or(0),
            running: st.tracking.is_some(),
            paused: st
                .tracking
                .as_ref()
                .map(|run| run.paused_ui)
                .unwrap_or(false),
            points: st.tracking.as_ref().map(|run| run.points).unwrap_or(0),
            confidence: st
                .tracking
                .as_ref()
                .map(|run| run.last_confidence)
                .unwrap_or(f32::NAN),
            status: st.status.clone().unwrap_or_default(),
        }
    };
    div()
        .size_full()
        .overflow_y_scrollbar()
        .flex()
        .flex_col()
        .gap_2()
        .p_4()
        .bg(c(0x0d1420))
        .text_color(c(0xdbe6f5))
        .child(dialog_header(&data))
        .child(dialog_actions(&data, state.clone(), cx))
        .child(dialog_preview(&data, template, matched, cx))
        .child(dialog_fields(&data, state.clone(), cx))
        .child(dialog_footer(&data, state, cx))
        .into_any_element()
}
