use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::*;

pub(crate) const ICON_DETACH: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M14 3h7v7M21 3l-9 9"/><path d="M10 5H5a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-5"/></svg>"#;
pub(crate) const ICON_DOCK: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 15h18M11 7l-4 4m0-4v4h4"/></svg>"#;
pub(crate) const ICON_MAXIMIZE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M9 3H3v6m0-6 7 7m5-7h6v6m0-6-7 7M3 15v6h6m-6 0 7-7m11 1v6h-6m6 0-7-7"/></svg>"#;
pub(crate) const ICON_RESTORE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 3l7 7m0-6v6H4m17-7-7 7m0-6v6h6M3 21l7-7m0 6v-6H4m17 7-7-7m0 6v-6h6"/></svg>"#;

pub(crate) const ICON_VIDEO: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M7 4v16M17 4v16M3 9h4m-4 6h4m10-6h4m-4 6h4"/></svg>"#;
pub(crate) const ICON_EYE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8"><path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12Z"/><circle cx="12" cy="12" r="3"/></svg>"#;
pub(crate) const ICON_EYE_OFF: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8"><path d="m3 3 18 18M9 5a12 12 0 0 1 3 0c6 0 10 7 10 7a22 22 0 0 1-4 4M6 6a22 22 0 0 0-4 6s4 7 10 7a12 12 0 0 0 6-2"/></svg>"#;
pub(crate) const ICON_TRACKS: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="6" cy="6" r="2"/><circle cx="6" cy="12" r="2"/><circle cx="6" cy="18" r="2"/><path d="M11 6h9"/><path d="M11 12h9"/><path d="M11 18h9"/></svg>"#;
// Toolbar icons redrawn as SVG from Tracker's bitmap icons.
pub(crate) const ICON_MOVE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3v18"/><path d="M3 12h18"/><path d="m9 6 3-3 3 3"/><path d="m9 18 3 3 3-3"/><path d="m6 9-3 3 3 3"/><path d="m18 9 3 3-3 3"/></svg>"#;
pub(crate) const ICON_RULER: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m3 17 14-14 4 4L7 21l-4-4Z"/><path d="m7 13 2 2"/><path d="m10 10 2 2"/><path d="m13 7 2 2"/></svg>"#;
pub(crate) const ICON_STEP: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M5 5v14l9-7-9-7Z"/><path d="M18 5v14"/></svg>"#;
pub(crate) const ICON_TARGET: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="7"/><circle cx="12" cy="12" r="2.2"/><path d="M12 2v3"/><path d="M12 19v3"/><path d="M2 12h3"/><path d="M19 12h3"/></svg>"#;
pub(crate) const ICON_MOTION: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4 8V4h4"/><path d="M16 4h4v4"/><path d="M20 16v4h-4"/><path d="M8 20H4v-4"/><circle cx="12" cy="12" r="3"/><path d="M12 5v4"/></svg>"#;
pub(crate) const ICON_ARUCO: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="4" y="4" width="16" height="16" rx="1.5"/><rect x="9" y="9" width="6" height="6"/><path d="M12 4v3"/><path d="M12 20v-3"/></svg>"#;
pub(crate) const ICON_ROI: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4 8V4h4"/><path d="M16 4h4v4"/><path d="M20 16v4h-4"/><path d="M8 20H4v-4"/><path d="M9 12h6"/></svg>"#;
pub(crate) const ICON_CONTOUR: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M5 16 3 9l6-4 7 2 5 6-4 5-7 1-5-3Z"/><circle cx="10" cy="11" r="1.3"/><circle cx="16" cy="13" r="1.3"/></svg>"#;
pub(crate) const ICON_REFRESH: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 0 1-15.5 6.2L3 16"/><path d="M3 21v-5h5"/><path d="M3 12a9 9 0 0 1 15.5-6.2L21 8"/><path d="M21 3v5h-5"/></svg>"#;
pub(crate) const ICON_FIRST: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M19 5v14"/><path d="m6 5 8 7-8 7V5Z"/></svg>"#;
pub(crate) const ICON_LAST: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M5 5v14"/><path d="m18 5-8 7 8 7V5Z"/></svg>"#;
pub(crate) const ICON_BACK10: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m11 17-5-5 5-5"/><path d="m18 17-5-5 5-5"/></svg>"#;
pub(crate) const ICON_FWD10: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m6 7 5 5-5 5"/><path d="m13 7 5 5-5 5"/></svg>"#;
pub(crate) const ICON_BACK1: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m15 18-6-6 6-6"/></svg>"#;
pub(crate) const ICON_FWD1: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="m9 18 6-6-6-6"/></svg>"#;
pub(crate) const ICON_PLAY: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M7 5v14l11-7Z"/></svg>"#;
pub(crate) const ICON_PAUSE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M8 5v14"/><path d="M16 5v14"/></svg>"#;
pub(crate) const ICON_ORIGIN: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3v18"/><path d="M3 12h18"/><circle cx="12" cy="12" r="3.2"/></svg>"#;
pub(crate) const ICON_ORIGIN_OFFSET: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4 20V5h15"/><path d="M10 15h8"/><path d="m15 12 3 3-3 3"/></svg>"#;
pub(crate) const ICON_AXES: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4 4v16h16"/><path d="M4 12h16"/><path d="M12 4v16"/><path d="M8 4v2"/><path d="M16 4v2"/><path d="M4 8h2"/><path d="M4 16h2"/></svg>"#;
pub(crate) const ICON_TAPE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M5 19 19 5"/><circle cx="5" cy="19" r="2"/><circle cx="19" cy="5" r="2"/><path d="m9 13 2 2"/><path d="m12 10 2 2"/></svg>"#;
pub(crate) const ICON_PROTRACTOR: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4 17a8 8 0 0 1 16 0"/><path d="M12 17V9"/><path d="M6 17h12"/><path d="m8 12-1 1"/><path d="M12 9v2"/><path d="m16 12 1 1"/></svg>"#;
pub(crate) const ICON_CIRCLE_FIT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="8"/><circle cx="4" cy="12" r="1.3"/><circle cx="20" cy="12" r="1.3"/><circle cx="12" cy="4" r="1.3"/></svg>"#;

pub(crate) const ICON_CLOSE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round"><path d="m6 6 12 12M18 6 6 18"/></svg>"#;

pub(crate) struct TooltipLabel {
    text: SharedString,
}

pub(crate) const ICON_SIDEBAR_LEFT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M9 4v16"/></svg>"#;
pub(crate) const ICON_SIDEBAR_RIGHT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M15 4v16"/></svg>"#;
pub(crate) const ICON_SIDEBAR_LEFT_OPEN: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M5 4h4v16H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2Z" fill='#000' stroke="none"/><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M9 4v16"/></svg>"#;
pub(crate) const ICON_SIDEBAR_RIGHT_OPEN: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke='#000' stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M15 4h4a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-4Z" fill='#000' stroke="none"/><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M15 4v16"/></svg>"#;

impl Render for TooltipLabel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(c(0x152238))
            .border_1()
            .border_color(c(0x2a3b55))
            .text_xs()
            .text_color(c(0xdbe6f5))
            .child(self.text.clone())
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn icon_button(
    id: &'static str,
    icon: &'static str,
    tooltip: impl Into<SharedString>,
    button_size: f32,
    icon_size: f32,
    active: bool,
    enabled: bool,
    cx: &mut Context<Probe>,
    action: impl Fn(&mut Probe, &mut Context<Probe>) + 'static,
) -> AnyElement {
    let tooltip = tooltip.into();
    let icon_color = if !enabled {
        c(0x4a5a73)
    } else if active {
        c(0x8ecbff)
    } else {
        c(0x9fb2cc)
    };
    div()
        .id(id)
        .flex_none()
        .w(px(button_size))
        .h(px(button_size))
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .bg(if active {
            c(0x1b2b45)
        } else {
            c(0x000000).alpha(0.0)
        })
        .text_color(icon_color)
        .when(enabled, |this| {
            this.cursor_pointer()
                .hover(|style| style.bg(c(0x152238)))
                .tooltip(move |_window, cx| {
                    cx.new(|_| TooltipLabel {
                        text: tooltip.clone(),
                    })
                    .into()
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _ev, _window, cx| {
                        this.state.borrow_mut().menu_open = None;
                        action(this, cx);
                        cx.notify();
                    }),
                )
        })
        .child(
            svg()
                .data(icon.as_bytes())
                .flex_none()
                .w(px(icon_size))
                .h(px(icon_size))
                .text_color(icon_color),
        )
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn group_icon_button(
    id: &'static str,
    icon: &'static str,
    tooltip: impl Into<SharedString>,
    group: ToolGroup,
    active: bool,
    enabled: bool,
    cx: &mut Context<Probe>,
    left_action: impl Fn(&mut Probe, &mut Context<Probe>) + 'static,
) -> AnyElement {
    let tooltip = tooltip.into();
    let icon_color = if !enabled {
        c(0x4a5a73)
    } else if active {
        c(0x8ecbff)
    } else {
        c(0x9fb2cc)
    };
    div()
        .id(id)
        .relative()
        .flex_none()
        .w(px(32.))
        .h(px(32.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .bg(if active {
            c(0x1b2b45)
        } else {
            c(0x000000).alpha(0.0)
        })
        .text_color(icon_color)
        .when(enabled, |this| {
            this.cursor_pointer()
                .hover(|style| style.bg(c(0x152238)))
                .tooltip(move |_window, cx| {
                    cx.new(|_| TooltipLabel {
                        text: tooltip.clone(),
                    })
                    .into()
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _ev, _window, cx| {
                        {
                            let mut st = this.state.borrow_mut();
                            st.tool_menu = None;
                        }
                        left_action(this, cx);
                        cx.notify();
                    }),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, ev: &MouseDownEvent, _window, cx| {
                        let mut st = this.state.borrow_mut();
                        st.menu_open = None;
                        st.tool_menu = Some(group);
                        st.tool_menu_pos = (f32::from(ev.position.x), f32::from(ev.position.y));
                        drop(st);
                        cx.notify();
                    }),
                )
        })
        .child(
            svg()
                .data(icon.as_bytes())
                .flex_none()
                .w(px(18.))
                .h(px(18.))
                .text_color(icon_color),
        )
        .when(enabled, |this| {
            this.child(
                div()
                    .absolute()
                    .right(px(1.))
                    .bottom(px(1.))
                    .text_color(icon_color)
                    .child("▾"),
            )
        })
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn menu_icon_button(
    id: &'static str,
    icon: &'static str,
    tooltip: impl Into<SharedString>,
    group: ToolGroup,
    active: bool,
    enabled: bool,
    cx: &mut Context<Probe>,
) -> AnyElement {
    let tooltip = tooltip.into();
    let icon_color = if !enabled {
        c(0x4a5a73)
    } else if active {
        c(0x8ecbff)
    } else {
        c(0x9fb2cc)
    };
    div()
        .id(id)
        .relative()
        .flex_none()
        .w(px(32.))
        .h(px(32.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .bg(if active {
            c(0x1b2b45)
        } else {
            c(0x000000).alpha(0.0)
        })
        .text_color(icon_color)
        .when(enabled, |this| {
            this.cursor_pointer()
                .hover(|style| style.bg(c(0x152238)))
                .tooltip(move |_window, cx| {
                    cx.new(|_| TooltipLabel {
                        text: tooltip.clone(),
                    })
                    .into()
                })
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, ev: &MouseDownEvent, _window, cx| {
                        let mut st = this.state.borrow_mut();
                        st.menu_open = None;
                        st.tool_menu = Some(group);
                        st.tool_menu_pos = (
                            f32::from(ev.position.x) + 8.0,
                            f32::from(ev.position.y) + 8.0,
                        );
                        drop(st);
                        cx.notify();
                    }),
                )
        })
        .child(
            svg()
                .data(icon.as_bytes())
                .flex_none()
                .w(px(18.))
                .h(px(18.))
                .text_color(icon_color),
        )
        .when(enabled, |this| {
            this.child(
                div()
                    .absolute()
                    .right(px(1.))
                    .bottom(px(1.))
                    .text_color(icon_color)
                    .child("▾"),
            )
        })
        .into_any_element()
}
