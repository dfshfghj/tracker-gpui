//! Standalone settings window: appearance, language and wheel bindings.
//!
//! Layout follows Zed's settings window (`crates/settings_ui`): a fixed header
//! bar, then a scrollable content column made of sections. Each section is a
//! small muted header with a hairline divider, followed by rows that put the
//! title and a one-line description on the left and the control on the right,
//! separated by hairlines.
//!
//! Every control writes straight into `Preferences` and saves; there is no
//! OK/Apply step. Theme and language are process-global, so those also call
//! `cx.refresh_windows()` to repaint the main window.

use std::cell::RefCell;
use std::rc::Rc;

use rust_i18n::t;

use crate::i18n::tr;
use crate::prefs::{TableCopyFormat, WheelBindings};
use crate::theme::{theme_mode, ThemeMode};
use crate::wheel::{modifier_spec, WheelAction};
use crate::*;
use gpui_component::select::{Select, SelectEvent, SelectItem, SelectState};
use gpui_component::Sizable;
use gpui_kit::prelude::FluentBuilder;

const BG: u32 = 0x070b12;
const HAIRLINE: u32 = 0x1d2a3d;
const TEXT: u32 = 0xdbe6f5;
const MUTED: u32 = 0x7d8ca6;
const FAINT: u32 = 0x5f6f88;
const CHIP: u32 = 0x111a29;
const CHIP_HOVER: u32 = 0x152238;
const CHIP_ACTIVE: u32 = 0x1b2b45;
const CHIP_ACTIVE_TEXT: u32 = 0x8ecbff;
const CHIP_TEXT: u32 = 0x9fb2cc;

pub(crate) struct SettingsWindow {
    state: Rc<RefCell<State>>,
    focus: FocusHandle,
    focused_once: bool,
    scroll: ScrollHandle,
    language: Entity<SelectState<Vec<SettingsChoice>>>,
    language_value: String,
    copy_format: Entity<SelectState<Vec<SettingsChoice>>>,
    copy_format_value: TableCopyFormat,
    recording: Option<WheelAction>,
    recorded_modifiers: Modifiers,
    recording_keys: HashSet<String>,
    recording_error: Option<String>,
}

impl SettingsWindow {
    pub(crate) fn new(
        state: Rc<RefCell<State>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let language_value = state.borrow().language_pref();
        let language = cx.new(|cx| {
            let mut select = SelectState::new(language_choices(), None, window, cx);
            select.set_selected_value(&language_value, window, cx);
            select
        });
        cx.subscribe(
            &language,
            |this, _, event: &SelectEvent<Vec<SettingsChoice>>, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    this.state.borrow_mut().apply_language(value);
                    cx.refresh_windows();
                }
            },
        )
        .detach();
        let copy_format_value = state.borrow().prefs.table_copy_format;
        let copy_format = cx.new(|cx| {
            let mut select = SelectState::new(copy_format_choices(), None, window, cx);
            select.set_selected_value(&copy_format_value.key().to_string(), window, cx);
            select
        });
        cx.subscribe(
            &copy_format,
            |this, _, event: &SelectEvent<Vec<SettingsChoice>>, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    let mut st = this.state.borrow_mut();
                    st.prefs.table_copy_format = if value == "csv" {
                        TableCopyFormat::Csv
                    } else {
                        TableCopyFormat::Excel
                    };
                    st.prefs.save();
                    drop(st);
                    cx.refresh_windows();
                }
            },
        )
        .detach();
        Self {
            state,
            focus: cx.focus_handle(),
            focused_once: false,
            scroll: ScrollHandle::new(),
            language,
            language_value,
            copy_format,
            copy_format_value,
            recording: None,
            recorded_modifiers: Modifiers::default(),
            recording_keys: HashSet::new(),
            recording_error: None,
        }
    }

    fn record_wheel(&mut self, ev: &ScrollWheelEvent, cx: &mut Context<Self>) {
        let Some(target) = self.recording else { return };
        cx.stop_propagation();
        let moved = match ev.delta {
            ScrollDelta::Pixels(p) => p.x != px(0.) || p.y != px(0.),
            ScrollDelta::Lines(p) => p.x != 0. || p.y != 0.,
        };
        if !moved {
            return;
        }
        if !self.recording_keys.is_empty() {
            self.recording_error = Some(tr("settings.wheel.unsupported"));
            cx.notify();
            return;
        }
        self.recorded_modifiers = ev.modifiers;
        let Some(value) = modifier_spec(&ev.modifiers) else {
            self.recording_error = Some(tr("settings.wheel.unsupported"));
            cx.notify();
            return;
        };
        let mut st = self.state.borrow_mut();
        if let Some(other) = st.prefs.wheel.conflict(target, &value) {
            self.recording_error = Some(
                t!(
                    "settings.wheel.conflict",
                    action = wheel_action_label(other)
                )
                .to_string(),
            );
        } else {
            st.prefs.wheel.set(target, value);
            st.prefs.save();
            self.recording = None;
            self.recording_error = None;
        }
        cx.notify();
    }

    fn cancel_recording(&mut self, cx: &mut Context<Self>) {
        self.recording = None;
        self.recording_keys.clear();
        self.recording_error = None;
        cx.notify();
    }

    fn wheel_control(
        &self,
        id: usize,
        action: WheelAction,
        bindings: &WheelBindings,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let active = self.recording == Some(action);
        let binding = bindings.get(action);
        let label = if active {
            modifier_spec(&self.recorded_modifiers)
                .map(|value| wheel_binding_label(&value))
                .unwrap_or_else(|| tr("settings.wheel.unsupported"))
        } else {
            wheel_binding_label(binding)
        };
        let field = div()
            .id(("settings-wheel-record", id))
            .w_full()
            .px_2()
            .py_1()
            .rounded_sm()
            .text_xs()
            .cursor_pointer()
            .border_1()
            .border_color(c(if active { 0x4dabf7 } else { CHIP }))
            .bg(c(if active { CHIP_ACTIVE } else { CHIP }))
            .text_color(c(if active { CHIP_ACTIVE_TEXT } else { CHIP_TEXT }))
            .hover(|style| style.bg(c(CHIP_HOVER)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
                    if this.recording != Some(action) {
                        this.recording = Some(action);
                        this.recorded_modifiers = ev.modifiers;
                        this.recording_keys.clear();
                        this.recording_error = None;
                    }
                    window.focus(&this.focus, cx);
                    cx.notify();
                }),
            )
            .child(label);
        let mut control = div()
            .w(px(190.))
            .flex()
            .flex_col()
            .items_end()
            .gap_1()
            .when(active, |control| {
                // The inline control owns scroll while recording; the content
                // behind it stays put.
                control
                    .occlude()
                    .on_scroll_wheel(cx.listener(|this, ev, _, cx| this.record_wheel(ev, cx)))
                    .on_mouse_down_out(cx.listener(move |this, _, _, cx| {
                        if this.recording == Some(action) {
                            this.cancel_recording(cx);
                        }
                    }))
            })
            .child(field);
        if active {
            control = control.child(
                div()
                    .w_full()
                    .text_xs()
                    .text_color(c(MUTED))
                    .child(tr("settings.wheel.record_hint")),
            );
        }
        let error = if active {
            self.recording_error.clone()
        } else {
            bindings.conflict(action, binding).map(|other| {
                t!(
                    "settings.wheel.conflict",
                    action = wheel_action_label(other)
                )
                .to_string()
            })
        };
        control
            .when_some(error, |control, error| {
                control.child(
                    div()
                        .w_full()
                        .text_xs()
                        .text_color(c(0xff6b6b))
                        .child(error),
                )
            })
            .into_any_element()
    }
}

#[derive(Clone)]
struct SettingsChoice {
    value: String,
    label: String,
}

impl SelectItem for SettingsChoice {
    type Value = String;
    fn title(&self) -> SharedString {
        self.label.clone().into()
    }
    fn value(&self) -> &String {
        &self.value
    }
}

fn language_choices() -> Vec<SettingsChoice> {
    crate::i18n::menu_entries()
        .into_iter()
        .map(|(value, label)| SettingsChoice {
            value: value.into(),
            label,
        })
        .collect()
}

fn copy_format_choices() -> Vec<SettingsChoice> {
    vec![
        SettingsChoice {
            value: "excel".into(),
            label: tr("settings.copy_excel"),
        },
        SettingsChoice {
            value: "csv".into(),
            label: "CSV".into(),
        },
    ]
}

fn wheel_action_label(action: WheelAction) -> String {
    match action {
        WheelAction::PanVertical => tr("settings.wheel.vertical"),
        WheelAction::PanHorizontal => tr("settings.wheel.horizontal"),
        WheelAction::Zoom => tr("settings.wheel.zoom"),
    }
}

/// One selectable chip: stored value, visible label, whether it is selected.
type Chip = (String, String, bool);

/// `Rc` because every chip needs its own copy of the callback.
type Pick = Rc<dyn Fn(&mut SettingsWindow, &str, &mut Context<SettingsWindow>)>;

fn chips(
    id_base: usize,
    items: Vec<Chip>,
    cx: &mut Context<SettingsWindow>,
    on_pick: Pick,
) -> AnyElement {
    let mut row = div()
        .flex()
        .items_center()
        .flex_wrap()
        .gap_1()
        .justify_end();
    for (index, (value, label, active)) in items.into_iter().enumerate() {
        let pick = on_pick.clone();
        row = row.child(
            div()
                .id(("settings-chip", id_base + index))
                .px_2()
                .py_0p5()
                .rounded_sm()
                .text_xs()
                .cursor_pointer()
                .bg(if active { c(CHIP_ACTIVE) } else { c(CHIP) })
                .text_color(if active {
                    c(CHIP_ACTIVE_TEXT)
                } else {
                    c(CHIP_TEXT)
                })
                .hover(|style| style.bg(c(CHIP_HOVER)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _ev, _window, cx| {
                        let value = value.clone();
                        pick(this, &value, cx);
                    }),
                )
                .child(label),
        );
    }
    row.into_any_element()
}

/// Section title with the hairline divider Zed puts under it.
fn section_header(title: String) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(c(FAINT))
                .child(title),
        )
        .child(div().h(px(1.)).w_full().bg(c(HAIRLINE)))
        .into_any_element()
}

/// One setting row: title + description on the left, control on the right.
fn setting_row(title: String, description: String, control: AnyElement, last: bool) -> AnyElement {
    let mut row = div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .py_2p5()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .flex_1()
                .min_w(px(0.))
                .child(div().text_sm().text_color(c(TEXT)).child(title))
                .child(div().text_xs().text_color(c(MUTED)).child(description)),
        )
        .child(div().flex_none().child(control));
    if !last {
        row = row.border_b_1().border_color(c(HAIRLINE));
    }
    row.into_any_element()
}

fn section(header: String, rows: Vec<AnyElement>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(section_header(header))
        .children(rows)
        .into_any_element()
}

/// Label shown for a wheel modifier value.
fn modifier_label(value: &str) -> String {
    if value.contains('+') {
        return value
            .split('+')
            .map(modifier_label)
            .collect::<Vec<_>>()
            .join(" + ");
    }
    // The glyphs ⌘/⌥ are missing from the bundled UI font, so spell the keys
    // out per platform instead.
    let mac = cfg!(target_os = "macos");
    match value {
        "none" => tr("settings.modifier.none"),
        "ctrl" => {
            if mac {
                tr("settings.modifier.cmd")
            } else {
                tr("settings.modifier.ctrl")
            }
        }
        "alt" => {
            if mac {
                tr("settings.modifier.option")
            } else {
                tr("settings.modifier.alt")
            }
        }
        "shift" => tr("settings.modifier.shift"),
        "ctrl+alt" => {
            if mac {
                tr("settings.modifier.cmd_option")
            } else {
                tr("settings.modifier.ctrl_alt")
            }
        }
        other => other.to_string(),
    }
}

fn wheel_binding_label(value: &str) -> String {
    if value == "none" {
        tr("settings.wheel.plain")
    } else {
        t!("settings.wheel.binding", modifiers = modifier_label(value)).to_string()
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused_once {
            self.focused_once = true;
            window.focus(&self.focus, cx);
        }

        if std::env::var_os("PROBE_SETTINGS_LOG").is_some() {
            eprintln!(
                "[settings] offset={:?} max={:?}",
                self.scroll.offset(),
                self.scroll.max_offset()
            );
        }

        let (theme, language, wheel) = {
            let st = self.state.borrow();
            (theme_mode(), st.language_pref(), st.prefs.wheel.clone())
        };

        // ---- appearance ----
        let theme_chips = chips(
            700,
            [
                ("auto", ThemeMode::Auto),
                ("light", ThemeMode::Light),
                ("dark", ThemeMode::Dark),
            ]
            .into_iter()
            .map(|(value, mode)| {
                let label = match mode {
                    ThemeMode::Auto => tr("menu.item.theme_auto"),
                    ThemeMode::Light => tr("menu.item.theme_light"),
                    ThemeMode::Dark => tr("menu.item.theme_dark"),
                };
                (value.to_string(), label, theme == mode)
            })
            .collect(),
            cx,
            Rc::new(
                |this: &mut SettingsWindow, value: &str, cx: &mut Context<SettingsWindow>| {
                    let mode = match value {
                        "light" => ThemeMode::Light,
                        "dark" => ThemeMode::Dark,
                        _ => ThemeMode::Auto,
                    };
                    this.state.borrow_mut().apply_theme_mode(mode);
                    crate::theme::sync_component_theme(cx);
                    cx.refresh_windows();
                },
            ),
        );

        // ---- language ----
        let language_changed = self.language_value != language;
        if language_changed {
            self.language.update(cx, |select, cx| {
                select.set_items(language_choices(), window, cx);
                select.set_selected_value(&language, window, cx);
            });
            self.language_value = language;
        }
        let language_select = Select::new(&self.language)
            .small()
            .w(px(190.))
            .into_any_element();
        let copy_format_value = self.state.borrow().prefs.table_copy_format;
        if self.copy_format_value != copy_format_value || language_changed {
            self.copy_format.update(cx, |select, cx| {
                select.set_items(copy_format_choices(), window, cx);
                select.set_selected_value(&copy_format_value.key().to_string(), window, cx);
            });
            self.copy_format_value = copy_format_value;
        }
        let copy_format_select = Select::new(&self.copy_format)
            .small()
            .w(px(190.))
            .into_any_element();
        // ---- wheel bindings ----
        let wheel_row = |id: usize,
                         title: &str,
                         description: &str,
                         action: WheelAction,
                         cx: &mut Context<Self>| {
            let control = self.wheel_control(id, action, &wheel, cx);
            setting_row(title.into(), description.into(), control, false)
        };
        let wheel_rows = vec![
            wheel_row(
                720,
                &tr("settings.wheel.vertical"),
                &tr("settings.wheel.vertical_desc"),
                WheelAction::PanVertical,
                cx,
            ),
            wheel_row(
                730,
                &tr("settings.wheel.horizontal"),
                &tr("settings.wheel.horizontal_desc"),
                WheelAction::PanHorizontal,
                cx,
            ),
            wheel_row(
                740,
                &tr("settings.wheel.zoom"),
                &tr("settings.wheel.zoom_desc"),
                WheelAction::Zoom,
                cx,
            ),
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .py_2()
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_xs()
                        .text_color(c(FAINT))
                        .child(tr("settings.wheel.hint")),
                )
                .child(
                    div()
                        .id("settings-wheel-reset")
                        .flex_none()
                        .px_2()
                        .py_0p5()
                        .rounded_sm()
                        .text_xs()
                        .cursor_pointer()
                        .bg(c(CHIP))
                        .text_color(c(CHIP_TEXT))
                        .hover(|style| style.bg(c(CHIP_HOVER)))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _ev, _window, cx| {
                                {
                                    let mut st = this.state.borrow_mut();
                                    st.prefs.wheel = WheelBindings::default();
                                    st.prefs.save();
                                }
                                cx.notify();
                            }),
                        )
                        .child(tr("settings.wheel.reset")),
                )
                .into_any_element(),
        ];

        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(c(BG))
            .text_color(c(TEXT))
            .track_focus(&self.focus)
            .key_context("Settings")
            .on_modifiers_changed(cx.listener(|this, ev: &ModifiersChangedEvent, _, cx| {
                if this.recording.is_some() {
                    this.recorded_modifiers = ev.modifiers;
                    cx.notify();
                }
            }))
            .capture_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                if this.recording.is_some() && ev.keystroke.key != "escape" {
                    cx.stop_propagation();
                    if !matches!(
                        ev.keystroke.key.as_str(),
                        "shift" | "control" | "ctrl" | "alt" | "cmd" | "super" | "platform"
                    ) {
                        this.recording_keys.insert(ev.keystroke.key.clone());
                        this.recording_error = Some(tr("settings.wheel.unsupported"));
                    }
                    cx.notify();
                }
            }))
            .capture_key_up(cx.listener(|this, ev: &KeyUpEvent, _, cx| {
                if this.recording.is_some() && ev.keystroke.key != "escape" {
                    this.recording_keys.remove(&ev.keystroke.key);
                    cx.stop_propagation();
                }
            }))
            .on_action(
                cx.listener(|_, _: &gpui_kit::base::actions::Cancel, window, _| {
                    window.remove_window();
                }),
            )
            // ---- scrollable content ----
            .child(
                div()
                    .id("settings-content")
                    .flex()
                    .flex_col()
                    .gap_5()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .px_6()
                    .pt_4()
                    .pb_6()
                    .child(section(
                        tr("settings.appearance"),
                        vec![setting_row(
                            tr("settings.theme"),
                            tr("settings.theme_desc"),
                            theme_chips,
                            true,
                        )],
                    ))
                    .child(section(
                        tr("settings.language"),
                        vec![setting_row(
                            tr("settings.language"),
                            tr("settings.language_desc"),
                            language_select,
                            true,
                        )],
                    ))
                    .child(section(
                        tr("settings.data_table"),
                        vec![setting_row(
                            tr("settings.copy_format"),
                            tr("settings.copy_format_desc"),
                            copy_format_select,
                            true,
                        )],
                    ))
                    .child(section(tr("settings.mouse_wheel"), wheel_rows))
                    .child(
                        div().text_xs().text_color(c(FAINT)).child(
                            t!("settings.saved_hint", path = prefs_path_label()).to_string(),
                        ),
                    ),
            )
    }
}

fn prefs_path_label() -> String {
    crate::prefs::Preferences::path()
        .map(|path| path.display().to_string())
        .unwrap_or_default()
}
