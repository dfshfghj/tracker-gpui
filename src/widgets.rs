use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::*;
use gpui_component::{
    input::{Input, InputState},
    Sizable,
};

#[cfg(test)]
std::thread_local! {pub(crate) static PROPERTY_INPUT_BOUNDS: RefCell<std::collections::HashMap<EntityId,Bounds<Pixels>>> = RefCell::new(std::collections::HashMap::new());}

// Native inputs retain focus on clicks that target non-focusable elements.
// Share one outside-click rule across property fields and tree renaming.
pub(crate) fn property_input<T: 'static>(
    id: &'static str,
    input: &Entity<InputState>,
    cx: &mut Context<T>,
) -> AnyElement {
    let field = input.clone();
    let wrapper = div()
        .id(id)
        .w_full()
        .min_w_0()
        .flex()
        .on_mouse_down_out(cx.listener(move |_, _, window, cx| {
            if field.read(cx).focus_handle(cx).is_focused(window) {
                window.blur(cx);
            }
        }));
    #[cfg(test)]
    let wrapper = {
        use gpui_kit::base::ElementExt;
        let id = input.entity_id();
        wrapper.on_prepaint(move |bounds, _, _| {
            PROPERTY_INPUT_BOUNDS.with(|map| {
                map.borrow_mut().insert(id, bounds);
            })
        })
    };
    wrapper
        .child(
            Input::new(input)
                .small()
                .w_full()
                .min_w_0()
                .focus_bordered(false)
                .bg(c(0x111a29))
                .text_color(c(0xdbe6f5))
                .border_color(c(0x1d2a3d)),
        )
        .into_any_element()
}

pub(crate) fn kv(label: impl Into<SharedString>, value: String) -> impl IntoElement {
    div()
        .flex()
        .justify_between()
        .text_xs()
        .text_color(c(0x7d8ca6))
        .child(label.into())
        .child(div().text_color(c(0xdbe6f5)).child(value))
}

pub(crate) fn format_confidence(value: f32) -> String {
    if value.is_finite() {
        format!("{value:.2}")
    } else {
        "—".to_string()
    }
}

pub(crate) fn plot_text(
    window: &mut Window,
    cx: &mut App,
    text: impl Into<SharedString>,
    x: f32,
    y: f32,
    size: f32,
    color: Rgba,
) {
    let text = text.into();
    let run = TextRun {
        len: text.len(),
        font: Font::default(),
        color: color.into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window
        .text_system()
        .shape_line(text.to_string().into(), px(size), &[run], None);
    let _ = line.paint(
        point(px(x), px(y)),
        px(size * 1.25),
        TextAlign::Left,
        None,
        window,
        cx,
    );
}

pub(crate) fn plot_chip(
    id: usize,
    label: String,
    active: bool,
    cx: &mut Context<Probe>,
    action: impl Fn(&mut Probe, &mut Context<Probe>) + 'static,
) -> AnyElement {
    div()
        .id(("plot-chip", id))
        .px_2()
        .py_0p5()
        .rounded_sm()
        .text_xs()
        .bg(if active { c(0x1b2b45) } else { c(0x111a29) })
        .text_color(if active { c(0x8ecbff) } else { c(0x9fb2cc) })
        .cursor_pointer()
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

pub(crate) fn check_item(
    id: usize,
    label: impl Into<SharedString>,
    checked: bool,
    enabled: bool,
    cx: &mut Context<Probe>,
    action: impl Fn(&mut Probe, &mut Context<Probe>) + 'static,
) -> AnyElement {
    let label = label.into();
    div()
        .id(("check-item", id))
        .flex()
        .items_center()
        .gap_1()
        .text_xs()
        .text_color(if enabled { c(0xdbe6f5) } else { c(0x5f6f88) })
        .when(enabled, |this| {
            this.cursor_pointer().on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _ev, _window, cx| {
                    action(this, cx);
                    cx.notify();
                }),
            )
        })
        .child(
            div()
                .w(px(13.))
                .h(px(13.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_sm()
                .border_1()
                .border_color(if checked { c(0x4dabf7) } else { c(0x5f6f88) })
                .bg(if checked {
                    c(0x4dabf7)
                } else {
                    c(0x000000).alpha(0.0)
                })
                .text_color(if checked { c(0x06101c) } else { c(0x5f6f88) })
                .child(if checked { "✓" } else { "" }),
        )
        .child(label)
        .into_any_element()
}

pub(crate) fn editable_value(
    state: Rc<RefCell<State>>,
    cx: &mut Context<Probe>,
    label: impl Into<SharedString>,
    target: InlineTarget,
    value: String,
    suffix: String,
    id: usize,
) -> AnyElement {
    let label = label.into();
    let editing = state
        .borrow()
        .inline_edit
        .as_ref()
        .map(|edit| edit.target == target)
        .unwrap_or(false);
    if editing {
        let (text, error) = {
            let st = state.borrow();
            st.inline_edit
                .as_ref()
                .map(|edit| (edit.text.clone(), edit.error.clone()))
                .unwrap_or_default()
        };
        return div()
            .id(("editing-property", id))
            .on_mouse_down_out(cx.listener(move |this, _, _, cx| {
                let mut state = this.state.borrow_mut();
                if state
                    .inline_edit
                    .as_ref()
                    .is_some_and(|edit| edit.target == target)
                {
                    if !state.apply_inline_edit() {
                        state.inline_edit = None;
                    }
                    cx.notify();
                }
            }))
            .flex()
            .flex_col()
            .gap_0p5()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .text_xs()
                    .text_color(c(0x7d8ca6))
                    .child(label.clone())
                    .child(
                        div()
                            .px_1()
                            .rounded_sm()
                            .border_1()
                            .border_color(c(0x4dabf7))
                            .bg(c(0x111a29))
                            .text_color(c(0xdbe6f5))
                            .child(format!("{text}▏")),
                    ),
            )
            .when_some(error, |this, error| {
                this.child(div().text_xs().text_color(c(0xff6b6b)).child(error))
            })
            .into_any_element();
    }
    let state_for_click = state.clone();
    let value_for_edit = value.clone();
    div()
        .id(("editable-value", id))
        .flex()
        .justify_between()
        .items_center()
        .text_xs()
        .text_color(c(0x7d8ca6))
        .cursor_pointer()
        .hover(|style| style.bg(c(0x152238)))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _ev, window, cx| {
                window.focus(&this.focus, cx);
                let mut st = state_for_click.borrow_mut();
                st.begin_inline_edit(target, value_for_edit.clone());
                drop(st);
                cx.notify();
            }),
        )
        .child(label)
        .child(
            div()
                .text_color(c(0xdbe6f5))
                .child(format!("{value}{suffix}  ✎")),
        )
        .into_any_element()
}

pub(crate) fn plot_var_chip(
    cx: &mut Context<Probe>,
    plot_index: usize,
    axis: PlotAxis,
    label: String,
) -> AnyElement {
    div()
        .id(("plot-var-chip", plot_index * 2 + axis as usize))
        .max_w(px(160.))
        .truncate()
        .px_2()
        .py_0p5()
        .rounded_sm()
        .text_xs()
        .bg(c(0x111a29))
        .text_color(c(0xdbe6f5))
        .cursor_pointer()
        .hover(|style| style.bg(c(0x152238)))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, ev: &MouseDownEvent, _window, cx| {
                let mut st = this.state.borrow_mut();
                st.menu_open = None;
                st.tool_menu = None;
                this.plot_var_menu = Some((plot_index, axis));
                this.plot_var_pos = (f32::from(ev.position.x), f32::from(ev.position.y));
                drop(st);
                cx.refresh_windows();
            }),
        )
        .child(label)
        .into_any_element()
}

pub(crate) fn format_time(seconds: f64) -> String {
    let seconds = seconds.max(0.0);
    let minutes = (seconds / 60.0).floor() as u32;
    let rest = seconds - minutes as f64 * 60.0;
    format!("{minutes:02}:{rest:04.1}")
}
