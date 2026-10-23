use gpui_component::scroll::{Scrollbar, ScrollbarHandle, ScrollbarMode};
use gpui_kit::*;

// Keep the rail in normal layout, outside the clipped content viewport. This
// works with both ordinary ScrollHandles and virtual lists without extra padding.
pub(crate) fn vertical_rail<H: ScrollbarHandle + Clone + 'static>(
    id: &'static str,
    handle: &H,
) -> Div {
    let rail = div()
        .relative()
        .flex_none()
        .w(Scrollbar::width())
        .min_h_0()
        .child(
            div().absolute().inset_0().child(
                Scrollbar::vertical(handle)
                    .id(id)
                    .viewport_from_layout()
                    .mode(ScrollbarMode::Always),
            ),
        );
    #[cfg(test)]
    let rail = observe_rail(id, rail);
    rail
}

pub(crate) fn vertical_frame<H: ScrollbarHandle + Clone + 'static>(
    id: &'static str,
    content: impl IntoElement,
    handle: &H,
) -> Div {
    div()
        .flex()
        .flex_1()
        .size_full()
        .min_w_0()
        .min_h_0()
        .overflow_hidden()
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .overflow_hidden()
                .child(content),
        )
        .child(vertical_rail(id, handle))
}

pub(crate) fn horizontal_rail(id: &'static str, handle: &ScrollHandle) -> Div {
    let rail = div()
        .relative()
        .flex_none()
        .h(Scrollbar::width())
        .min_w_0()
        .child(
            div().absolute().inset_0().child(
                Scrollbar::horizontal(handle)
                    .id(id)
                    .viewport_from_layout()
                    .mode(ScrollbarMode::Always),
            ),
        );
    #[cfg(test)]
    let rail = observe_rail(id, rail);
    rail
}

#[cfg(test)]
std::thread_local! {
    static RAIL_BOUNDS: std::cell::RefCell<std::collections::HashMap<&'static str, Bounds<Pixels>>> = Default::default();
}

#[cfg(test)]
fn observe_rail(id: &'static str, rail: Div) -> Div {
    use gpui_kit::base::ElementExt;
    rail.on_prepaint(move |bounds, _, _| {
        RAIL_BOUNDS.with(|map| map.borrow_mut().insert(id, bounds));
    })
}

#[derive(IntoElement)]
pub(crate) struct ScrollArea {
    id: &'static str,
    content: AnyElement,
}

pub(crate) fn vertical(id: &'static str, content: impl IntoElement) -> ScrollArea {
    ScrollArea {
        id,
        content: content.into_any_element(),
    }
}

impl RenderOnce for ScrollArea {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let handle = window
            .use_keyed_state(self.id, cx, |_, _| ScrollHandle::default())
            .read(cx)
            .clone();
        let viewport = div()
            .id((ElementId::from(self.id), "viewport"))
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&handle)
            .child(self.content);
        vertical_frame(self.id, viewport, &handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{prefs::Preferences, Probe, State, Track};
    use ::gpui;
    use core::prelude::v1::test;
    use std::{cell::RefCell, rc::Rc};

    struct Host {
        probe: Entity<Probe>,
        ordinary: ScrollHandle,
    }
    impl Render for Host {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let viewport = div()
                .id("ordinary-viewport")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&self.ordinary)
                .child(div().w_full().h(px(2000.)).flex_none());
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(div().h(px(200.)).flex_none().flex().child(vertical_frame(
                    "ordinary-rail",
                    viewport,
                    &self.ordinary,
                )))
                .child(self.probe.update(cx, |probe, cx| {
                    let state = probe.state.borrow();
                    crate::data_table::table_view(
                        &state,
                        &probe.table_focus,
                        &probe.table_scroll,
                        &probe.table_x_scroll,
                        cx,
                    )
                }))
        }
    }

    #[gpui::test]
    fn rails_reserve_space_and_stay_visible_for_a_wide_virtual_table(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let state = Rc::new(RefCell::new(
            State::with_preferences(Preferences::default()),
        ));
        let mut track = Track::point_mass(1, "Track", 0);
        for frame in 0..10000 {
            track.set_point(frame, frame as f32, 1.);
        }
        state.borrow_mut().tracks.push(track);
        state.borrow_mut().table_columns = [true; 5];
        let probe = cx.new(|cx| {
            Probe::analysis_instance(
                state.clone(),
                None,
                Rc::new(RefCell::new(crate::analysis::PaneWindows::default())),
                cx,
            )
        });
        let ordinary = ScrollHandle::default();
        let (handle, host) = cx
            .update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds {
                            origin: point(px(0.), px(0.)),
                            size: size(px(240.), px(600.)),
                        })),
                        ..Default::default()
                    },
                    cx,
                    |_, cx| {
                        cx.new(|_| Host {
                            probe: probe.clone(),
                            ordinary: ordinary.clone(),
                        })
                    },
                )
            })
            .unwrap();
        handle
            .update(cx, |_, window, cx| {
                let _ = window.draw(cx);
            })
            .unwrap();
        let check = || {
            RAIL_BOUNDS.with(|map| {
                let map = map.borrow();
                let rail = map["ordinary-rail"];
                let viewport = ordinary.viewport_bounds();
                assert!(viewport.right() <= rail.left());
                assert_eq!(rail.size.width, Scrollbar::width());
                assert_eq!(rail.size.height, viewport.size.height);
                let vertical = map["table-vertical-scrollbar"];
                let horizontal = map["table-horizontal-scrollbar"];
                assert!(vertical.right() <= px(240.));
                assert!(vertical.size.height > px(200.));
                assert!(horizontal.bottom() <= px(600.));
                assert_eq!(horizontal.right(), vertical.left());
                assert_eq!(horizontal.top(), vertical.bottom());
            });
        };
        check();
        probe.update(cx, |probe, _| {
            assert!(probe.table_scroll.0.borrow().base_handle.max_offset().y > px(10000.));
            assert!(probe.table_x_scroll.max_offset().x > px(100.));
            probe.table_x_scroll.set_offset(point(px(-100.), px(0.)));
            probe
                .table_scroll
                .scroll_to_item(9999, ScrollStrategy::Bottom);
        });
        host.update(cx, |_, cx| cx.notify());
        handle
            .update(cx, |_, window, cx| {
                let _ = window.draw(cx);
            })
            .unwrap();
        check();
        probe.update(cx, |probe, _| {
            assert!(probe.table_scroll.0.borrow().base_handle.offset().y < px(-10000.));
        });
        handle
            .update(cx, |_, window, _| window.remove_window())
            .unwrap();
    }
}
