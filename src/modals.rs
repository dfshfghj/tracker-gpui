use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::i18n::tr;
use rust_i18n::t;
use crate::*;

pub(crate) fn modal_button(
    label: impl Into<SharedString>,
    primary: bool,
    cx: &mut Context<Probe>,
    action: impl Fn(&mut Probe, &mut Context<Probe>) + 'static,
) -> AnyElement {
    let label = label.into();
    div()
        .id(label.clone())
        .px_3()
        .py_1()
        .rounded_sm()
        .cursor_pointer()
        .text_xs()
        .bg(if primary { c(0x4dabf7) } else { c(0x111a29) })
        .text_color(if primary { c(0x06101c) } else { c(0xdbe6f5) })
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

pub(crate) fn calibration_field_box(
    label: impl Into<SharedString>,
    value: String,
    active: bool,
    field: CalibrationField,
    state: Rc<RefCell<State>>,
    cx: &mut Context<Probe>,
) -> AnyElement {
    let label = label.into();
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .w(px(52.))
                .text_xs()
                .text_color(c(0x7d8ca6))
                .child(label.clone()),
        )
        .child(
            div()
                .id(label)
                .flex_1()
                .px_2()
                .py_1()
                .rounded_sm()
                .bg(if active { c(0x16233a) } else { c(0x111a29) })
                .border_1()
                .border_color(if active { c(0x4dabf7) } else { c(0x1d2a3d) })
                .text_xs()
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |_this, _ev, _window, cx| {
                        state.borrow_mut().cal_field = field;
                        cx.notify();
                    }),
                )
                .child(format!("{value}{}", if active { "▏" } else { "" })),
        )
        .into_any_element()
}

pub(crate) fn calibration_modal_view(
    state: Rc<RefCell<State>>,
    cx: &mut Context<Probe>,
) -> AnyElement {
    let (length, unit, decimals, field, error, locked, active_stick, stick_count) = {
        let st = state.borrow();
        (
            st.cal_length_input.clone(),
            st.cal_unit_input.clone(),
            st.cal_decimals_input.clone(),
            st.cal_field,
            st.cal_error.clone(),
            st.coords.locked,
            st.active_stick,
            st.calibration.stick_count(),
        )
    };
    let description = t!(
        "cal.description",
        index = active_stick + 1,
        count = stick_count.max(1)
    )
    .to_string();

    let rows = vec![
        calibration_field_box(
            tr("cal.length"),
            length,
            field == CalibrationField::Length,
            CalibrationField::Length,
            state.clone(),
            cx,
        ),
        calibration_field_box(
            tr("cal.unit"),
            unit,
            field == CalibrationField::Unit,
            CalibrationField::Unit,
            state.clone(),
            cx,
        ),
        calibration_field_box(
            tr("cal.decimals"),
            decimals,
            field == CalibrationField::Decimals,
            CalibrationField::Decimals,
            state.clone(),
            cx,
        ),
    ];

    let apply_state = state.clone();
    let cancel_state = state.clone();
    crate::overlay::overlay_surface()
        .absolute()
        .top(px(0.))
        .left(px(0.))
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(c(0x000000).alpha(0.55))
        .child(
            div()
                .w(px(480.))
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .rounded_md()
                .bg(c(0x0d1420))
                .border_1()
                .border_color(c(0x2a3b55))
                .text_color(c(0xdbe6f5))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .child(tr("cal.title")),
                )
                .child(div().text_xs().text_color(c(0x7d8ca6)).child(description))
                .children(rows)
                .when(locked, |this| {
                    this.child(
                        div()
                            .text_xs()
                            .text_color(c(0xffd43b))
                            .child(tr("cal.locked")),
                    )
                })
                .when_some(error, |this, error| {
                    this.child(div().text_xs().text_color(c(0xff6b6b)).child(error))
                })
                .child(
                    div()
                        .text_xs()
                        .text_color(c(0x5f6f88))
                        .child(tr("cal.hint")),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(modal_button(tr("dlg.apply"), true, cx, move |_this, _cx| {
                            apply_state.borrow_mut().apply_calibration_dialog();
                        }))
                        .child(modal_button(tr("dlg.cancel"), false, cx, move |_this, _cx| {
                            let mut st = cancel_state.borrow_mut();
                            st.modal = None;
                            st.cal_error = None;
                        })),
                ),
        )
        .into_any_element()
}

pub(crate) fn operations_modal_view(
    state: Rc<RefCell<State>>,
    cx: &mut Context<Probe>,
) -> AnyElement {
    let rows = [
        (tr("help.play"), tr("help.play_keys")),
        (tr("help.frame"), tr("help.frame_keys")),
        (
            tr("help.move"),
            tr("help.move_keys"),
        ),
        (tr("help.select"), tr("help.select_keys")),
        (tr("help.object_props"), tr("help.object_props_keys")),
        (
            tr("help.calib"),
            tr("help.calib_keys"),
        ),
        (
            tr("help.point_track"),
            tr("help.point_track_keys"),
        ),
        (tr("help.autostep"), tr("help.autostep_keys")),
        (
            tr("help.trail"),
            tr("help.trail_keys"),
        ),
        (
            tr("help.autotracker"),
            tr("help.autotracker_keys"),
        ),
        (
            tr("help.correction"),
            tr("help.correction_keys"),
        ),
        (
            tr("help.tabs"),
            tr("help.tabs_keys"),
        ),
        (tr("help.plot"), tr("help.plot_keys")),
        (tr("help.table"), tr("help.table_keys")),
        (
            tr("help.delete"),
            tr("help.delete_keys"),
        ),
        (tr("help.undo"), tr("help.undo_keys")),
        (tr("help.zoom"), tr("help.zoom_keys")),
        (tr("help.tabbar"), tr("help.tabbar_keys")),
        (
            tr("help.sidebar"),
            tr("help.sidebar_keys"),
        ),
    ];
    let close_state = state.clone();
    let mut body = div().flex().flex_col().gap_2();
    for (action, keys) in rows {
        body = body.child(
            div()
                .flex()
                .gap_3()
                .text_xs()
                .child(
                    div()
                        .w(px(108.))
                        .flex_none()
                        .text_color(c(0xdbe6f5))
                        .child(action),
                )
                .child(div().flex_1().text_color(c(0x9fb2cc)).child(keys)),
        );
    }
    crate::overlay::overlay_surface()
        .absolute()
        .top(px(0.))
        .left(px(0.))
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(c(0x000000).alpha(0.55))
        .child(
            div()
                .w(px(450.))
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .rounded_md()
                .bg(c(0x0d1420))
                .border_1()
                .border_color(c(0x2a3b55))
                .text_color(c(0xdbe6f5))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .child(tr("help.title")),
                )
                .child(body)
                .child(div().flex().justify_end().child(modal_button(
                    tr("dlg.close"),
                    true,
                    cx,
                    move |_this, _cx| {
                        close_state.borrow_mut().modal = None;
                    },
                ))),
        )
        .into_any_element()
}

pub(crate) fn modal_view(state: Rc<RefCell<State>>, cx: &mut Context<Probe>) -> AnyElement {
    let Some(modal) = state.borrow().modal.clone() else {
        return div().into_any_element();
    };
    if matches!(modal, Modal::Calibration) {
        return calibration_modal_view(state, cx);
    }
    if matches!(modal, Modal::Operations) {
        return operations_modal_view(state, cx);
    }

    let mut buttons = Vec::new();
    let (title, message): (String, String) = match &modal {
        Modal::CloseDirty { index } => {
            let name = state
                .borrow()
                .docs
                .get(*index)
                .map(|doc| doc.title.clone())
                .unwrap_or_else(|| tr("dlg.video"));
            let index = *index;
            buttons.push(modal_button(tr("dlg.save"), true, cx, move |this, cx| {
                request_save(this.state.clone(), SaveAction::CloseDoc(index), cx);
            }));
            buttons.push(modal_button(tr("dlg.discard"), false, cx, move |this, _cx| {
                let mut st = this.state.borrow_mut();
                st.close_doc(index);
                st.modal = None;
            }));
            buttons.push(modal_button(tr("dlg.cancel"), false, cx, move |this, _cx| {
                this.state.borrow_mut().modal = None;
            }));
            (tr("dlg.unsaved_title"), t!("dlg.unsaved_body", name = name).to_string())
        }
        Modal::ConfirmNew => {
            buttons.push(modal_button(
                tr("dlg.save_and_new"),
                true,
                cx,
                move |this, cx| {
                    request_save(this.state.clone(), SaveAction::NewProject, cx);
                },
            ));
            buttons.push(modal_button(
                tr("dlg.discard_and_new"),
                false,
                cx,
                move |this, _cx| {
                    let mut st = this.state.borrow_mut();
                    st.new_project();
                    st.modal = None;
                },
            ));
            buttons.push(modal_button(tr("dlg.cancel"), false, cx, move |this, _cx| {
                this.state.borrow_mut().modal = None;
            }));
            (
                tr("dlg.new_project"),
                tr("dlg.unsaved_prompt"),
            )
        }
        Modal::ConfirmOpenProject(path) => {
            let path_for_save = path.clone();
            buttons.push(modal_button(
                tr("dlg.save_and_open"),
                true,
                cx,
                move |this, cx| {
                    request_save(
                        this.state.clone(),
                        SaveAction::OpenProject(path_for_save.clone()),
                        cx,
                    );
                },
            ));
            let path_for_discard = path.clone();
            buttons.push(modal_button(
                tr("dlg.discard_and_open"),
                false,
                cx,
                move |this, cx| {
                    open_project_file(this.state.clone(), path_for_discard.clone(), cx);
                },
            ));
            buttons.push(modal_button(tr("dlg.cancel"), false, cx, move |this, _cx| {
                this.state.borrow_mut().modal = None;
            }));
            (
                tr("dlg.open_project"),
                tr("dlg.unsaved_prompt"),
            )
        }
        Modal::About => {
            buttons.push(modal_button(tr("dlg.close"), true, cx, move |this, _cx| {
                this.state.borrow_mut().modal = None;
            }));
            (
                tr("dlg.about_title"),
                "Tracker GPUI · FFmpeg 9 · gpui-kit 0.7.3".to_string(),
            )
        }
        Modal::Error(message) => {
            buttons.push(modal_button(tr("dlg.close"), true, cx, move |this, _cx| {
                this.state.borrow_mut().modal = None;
            }));
            (tr("dlg.error"), message.clone())
        }
        Modal::Calibration | Modal::Operations => unreachable!(),
    };

    crate::overlay::overlay_surface()
        .absolute()
        .top(px(0.))
        .left(px(0.))
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(c(0x000000).alpha(0.55))
        .child(
            div()
                .w(px(460.))
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .rounded_md()
                .bg(c(0x0d1420))
                .border_1()
                .border_color(c(0x2a3b55))
                .text_color(c(0xdbe6f5))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .child(title),
                )
                .child(div().text_xs().text_color(c(0x9fb2cc)).child(message))
                .child(div().flex().justify_end().gap_2().children(buttons)),
        )
        .into_any_element()
}
