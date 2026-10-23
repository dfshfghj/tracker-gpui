use std::{cell::RefCell, rc::Rc};

use gpui_component::input::{InputEvent, InputState};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::{c, i18n::tr, plot::PlotVar, variables::Variables, State};

pub(crate) struct VariableEditor {
    state: Rc<RefCell<State>>,
    name: Entity<InputState>,
    expression: Entity<InputState>,
    context: Option<(u64, u64)>,
    editing: Option<u64>,
    open: bool,
    error: Option<String>,
}

impl VariableEditor {
    pub(crate) fn new(
        state: Rc<RefCell<State>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("r"));
        let expression = cx.new(|cx| InputState::new(window, cx).placeholder("sqrt(x^2 + y^2)"));
        for input in [&name, &expression] {
            cx.subscribe(input, |this, _, _: &InputEvent, cx| {
                this.error = None;
                cx.notify();
            })
            .detach();
        }
        Self {
            state,
            name,
            expression,
            context: None,
            editing: None,
            open: true,
            error: None,
        }
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing = None;
        self.error = None;
        self.name.update(cx, |s, cx| s.set_value("", window, cx));
        self.expression
            .update(cx, |s, cx| s.set_value("", window, cx));
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.name.read(cx).value().to_string();
        let expression = self.expression.read(cx).value().to_string();
        let result = {
            let mut st = self.state.borrow_mut();
            let Some(track) = st.active_track_mut() else {
                return;
            };
            let result = track.set_variable(self.editing, &name, &expression);
            if result.is_ok() {
                st.sync_table_selection();
                st.sync_plot_for_active();
                st.commit_edit();
            }
            result
        };
        match result {
            Ok(_) => {
                self.reset(window, cx);
                cx.refresh_windows();
            }
            Err(error) => {
                self.error = Some(error);
                cx.notify();
            }
        }
    }
}

impl Render for VariableEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (context, count) = {
            let st = self.state.borrow();
            let Some(track) = st.active_track().filter(|t| t.is_point_mass()) else {
                return div();
            };
            ((st.id, track.id), track.variables.len())
        };
        let header = div()
            .id("variable-editor-toggle")
            .cursor_pointer()
            .py_1()
            .text_xs()
            .text_color(c(0x8ecbff))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.open = !this.open;
                    cx.notify();
                }),
            )
            .child(format!(
                "{} {} ({count})",
                if self.open { "▾" } else { "▸" },
                tr("variables.properties")
            ));
        if !self.open {
            return div().w_full().child(header);
        }
        // Keep the immutable document borrowed while building controls; cloning
        // an entire track here would copy all samples on every inspector redraw.
        let state = self.state.clone();
        let st = state.borrow();
        let track = st.active_track().unwrap();
        let unit = st.calibration.unit.clone();

        if self.context != Some(context) {
            self.context = Some(context);
            self.reset(window, cx);
        }
        if self
            .editing
            .is_some_and(|id| !track.variables.iter().any(|d| d.id == id))
        {
            self.reset(window, cx);
        }
        let compiled = Variables::compile(&track);
        let name = self.name.read(cx).value().to_string();
        let expression = self.expression.read(cx).value().to_string();
        let preview = if !name.trim().is_empty() && !expression.trim().is_empty() {
            let mut candidate = track.clone();
            Some(
                candidate
                    .set_variable(self.editing, &name, &expression)
                    .and_then(|id| {
                        Ok(Variables::compile(&candidate)?
                            .unit(id, &unit)
                            .unwrap_or_default())
                    }),
            )
        } else {
            None
        };
        let mut body = div().w_full().flex().flex_col().gap_2();
        for def in &track.variables {
            let id = def.id;
            let label = compiled.as_ref().map_or_else(
                |_| PlotVar::Custom(id).axis_label_for(&track, &unit),
                |vars| vars.axis_label(PlotVar::Custom(id), &track, &unit),
            );
            let value = compiled
                .as_ref()
                .ok()
                .and_then(|vars| {
                    vars.value(
                        PlotVar::Custom(id),
                        track,
                        st.frame,
                        &st.coords,
                        st.video
                            .as_ref()
                            .map(|v| v.meta.frame_times.as_slice())
                            .unwrap_or(&[]),
                    )
                })
                .filter(|v| v.is_finite())
                .map(|v| format!("{:.*}", st.calibration.decimals, v))
                .unwrap_or_else(|| "-".into());
            let edit_name = def.name.clone();
            let edit_expression = def.expression.clone();
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(c(0xa9bad2))
                            .child(format!("{label} = {value}"))
                            .child(
                                div()
                                    .text_color(c(0x7d8ca6))
                                    .child(format!("= {}", def.expression)),
                            ),
                    )
                    .child(
                        div()
                            .id(("edit-variable", id))
                            .cursor_pointer()
                            .text_xs()
                            .text_color(c(0x8ecbff))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    this.editing = Some(id);
                                    this.error = None;
                                    this.name.update(cx, |s, cx| {
                                        s.set_value(edit_name.clone(), window, cx)
                                    });
                                    this.expression.update(cx, |s, cx| {
                                        s.set_value(edit_expression.clone(), window, cx)
                                    });
                                    cx.notify();
                                }),
                            )
                            .child(tr("variables.edit")),
                    )
                    .child(
                        div()
                            .id(("delete-variable", id))
                            .cursor_pointer()
                            .text_xs()
                            .text_color(c(0x8ecbff))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    let mut st = this.state.borrow_mut();
                                    let result =
                                        st.active_track_mut().map(|t| t.remove_variable(id));
                                    if let Some(Err(error)) = result {
                                        this.error = Some(error);
                                    } else {
                                        st.sync_plot_for_active();
                                        st.sync_table_selection();
                                        st.commit_edit();
                                        this.error = None;
                                    }
                                    drop(st);
                                    cx.refresh_windows();
                                }),
                            )
                            .child(tr("variables.delete")),
                    ),
            );
        }
        body = body
            .child(
                div()
                    .text_xs()
                    .text_color(c(0x7d8ca6))
                    .child(tr("variables.name")),
            )
            .child(crate::widgets::property_input(
                "custom-property-name",
                &self.name,
                cx,
            ))
            .child(
                div()
                    .text_xs()
                    .text_color(c(0x7d8ca6))
                    .child(tr("variables.expression")),
            )
            .child(crate::widgets::property_input(
                "custom-property-expression",
                &self.expression,
                cx,
            ));
        let message = self
            .error
            .clone()
            .or_else(|| compiled.err())
            .or_else(|| preview.as_ref().and_then(|r| r.as_ref().err().cloned()));
        if let Some(message) = message {
            body = body.child(div().text_xs().text_color(c(0xff9090)).child(message));
        } else if let Some(Ok(unit)) = preview {
            body = body.child(
                div().text_xs().text_color(c(0x8ecbff)).child(
                    rust_i18n::t!(
                        "variables.unit",
                        detail = if unit.is_empty() {
                            tr("variables.dimensionless")
                        } else {
                            unit
                        }
                    )
                    .to_string(),
                ),
            );
        }
        body = body.child(
            div()
                .flex()
                .gap_3()
                .child(
                    div()
                        .id("save-variable")
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .bg(c(0x1b2b45))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(c(0x8ecbff))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, window, cx| this.save(window, cx)),
                        )
                        .child(tr("variables.save")),
                )
                .when(self.editing.is_some(), |row| {
                    row.child(
                        div()
                            .id("cancel-variable")
                            .cursor_pointer()
                            .text_xs()
                            .text_color(c(0x7d8ca6))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.reset(window, cx);
                                    cx.notify();
                                }),
                            )
                            .child(tr("variables.cancel")),
                    )
                }),
        );
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .child(header)
            .child(body)
    }
}
