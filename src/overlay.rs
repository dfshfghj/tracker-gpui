//! Pointer boundaries shared by hand-rendered menus and modal backdrops.
use gpui_kit::*;

/// Visual stacking alone does not block GPUI hit testing. Every floating surface
/// (including transparent dismissal backdrops) must own its covered pointer area.
/// Children are painted after this hitbox, so controls inside remain interactive.
pub(crate) fn overlay_surface() -> Div {
    div().occlude()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::gpui;
    // The GPUI glob also exports its `test` macro. Its expansion needs Rust's
    // built-in attribute in scope rather than recursively invoking itself.
    use core::prelude::v1::test;
    use gpui_kit::prelude::FluentBuilder;
    use std::{cell::Cell, rc::Rc};

    struct Harness {
        open: bool,
        background: Rc<Cell<usize>>,
        background_motion: Rc<Cell<usize>>,
        selected: Rc<Cell<usize>>,
        dismissed: Rc<Cell<usize>>,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let background = self.background.clone();
            let motion = self.background_motion.clone();
            let scroll = self.background_motion.clone();
            div()
                .relative()
                .size(px(200.))
                .child(
                    div()
                        .absolute()
                        .size_full()
                        .on_any_mouse_down(move |_, _, _| {
                            background.set(background.get() + 1);
                        })
                        .on_mouse_move(move |_, _, _| motion.set(motion.get() + 1))
                        .on_scroll_wheel(move |_, _, _| scroll.set(scroll.get() + 1)),
                )
                .when(self.open, |root| {
                    root.child(overlay_surface().absolute().size_full().on_any_mouse_down(
                        cx.listener(|this, _, _, cx| {
                            this.dismissed.set(this.dismissed.get() + 1);
                            this.open = false;
                            cx.notify();
                        }),
                    ))
                    .child(
                        overlay_surface()
                            .absolute()
                            .top(px(20.))
                            .left(px(20.))
                            .size(px(100.))
                            .child(div().size(px(40.)).on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.selected.set(this.selected.get() + 1);
                                    this.open = false;
                                    cx.notify();
                                }),
                            )),
                    )
                })
        }
    }

    #[gpui::test]
    fn popup_selection_padding_and_dismissal_do_not_click_through(cx: &mut TestAppContext) {
        let background = Rc::new(Cell::new(0));
        let background_motion = Rc::new(Cell::new(0));
        let selected = Rc::new(Cell::new(0));
        let dismissed = Rc::new(Cell::new(0));
        let (view, window) = cx.add_window_view({
            let background = background.clone();
            let background_motion = background_motion.clone();
            let selected = selected.clone();
            let dismissed = dismissed.clone();
            move |_, _| Harness {
                open: true,
                background,
                background_motion,
                selected,
                dismissed,
            }
        });
        window.update(|window, cx| window.draw(cx).clear(cx));
        // A disabled item / padding still owns the pointer, for every button.
        for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
            window.simulate_mouse_down(point(px(100.), px(100.)), button, Modifiers::default());
            window.simulate_mouse_up(point(px(100.), px(100.)), button, Modifiers::default());
        }
        assert_eq!(background.get(), 0, "popup padding clicked the background");
        assert_eq!(
            dismissed.get(),
            0,
            "popup padding clicked the dismissal layer"
        );

        window.simulate_mouse_move(point(px(100.), px(100.)), None, Modifiers::default());
        window.simulate_event(ScrollWheelEvent {
            position: point(px(100.), px(100.)),
            delta: ScrollDelta::Lines(point(0., -1.)),
            ..Default::default()
        });
        assert_eq!(background_motion.get(), 0, "popup leaked hover or scroll");

        // The item closes its popup on down; its event must still be isolated.
        window.simulate_click(point(px(30.), px(30.)), Modifiers::default());
        assert_eq!(selected.get(), 1);
        assert_eq!(dismissed.get(), 0);
        assert_eq!(background.get(), 0);
        window.update(|window, cx| window.draw(cx).clear(cx));
        // No timer may swallow the next deliberate workspace click.
        window.simulate_click(point(px(30.), px(30.)), Modifiers::default());
        assert_eq!(background.get(), 1);

        window.update(|window, cx| {
            view.update(cx, |this, cx| {
                this.open = true;
                cx.notify();
                window.refresh();
            })
        });
        window.update(|window, cx| window.draw(cx).clear(cx));
        window.simulate_click(point(px(150.), px(150.)), Modifiers::default());
        assert_eq!(dismissed.get(), 1);
        assert_eq!(
            background.get(),
            1,
            "outside dismissal clicked the workspace"
        );
    }
}
