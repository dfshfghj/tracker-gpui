//! Mapping wheel / touchpad scrolling onto canvas actions.
//!
//! The bindings are user-configurable (`Preferences::wheel`) because a plain
//! wheel is vertical-only on a mouse: horizontal panning has to borrow the
//! vertical delta, and zoom needs an explicit modifier so ordinary scrolling
//! keeps working.

use gpui_kit::Modifiers;

use crate::prefs::WheelBindings;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WheelAction {
    /// Scale around the cursor.
    Zoom,
    /// Pan sideways.
    PanHorizontal,
    /// Pan up/down, and sideways too when the device reports both axes.
    PanVertical,
}

/// Pixels of canvas movement per unit of scroll delta.
///
/// One wheel notch arrives as ~3 lines (~60 px after the `Lines` conversion in
/// the scroll handler), which felt far too fast for panning; zoom keeps its own
/// (much smaller) rate because it is exponential.
pub(crate) const PAN_SPEED: f32 = 0.4;

/// Exponent applied to the scroll delta when zooming.
pub(crate) const ZOOM_SPEED: f32 = 0.0015;

/// Which action a scroll event with `mods` should trigger.
///
/// Bindings match exactly. Unbound combinations do not change the canvas;
/// duplicate legacy settings use zoom > horizontal > vertical priority.
pub(crate) fn action(mods: &Modifiers, bindings: &WheelBindings) -> Option<WheelAction> {
    let spec = modifier_spec(mods)?;
    // Deterministic handling for old preferences containing duplicate bindings.
    [
        WheelAction::Zoom,
        WheelAction::PanHorizontal,
        WheelAction::PanVertical,
    ]
    .into_iter()
    .find(|&action| bindings.get(action) == spec)
}

/// Canonical modifier names shared by recording, conflict detection and dispatch.
/// `ctrl` denotes Cmd on macOS and Ctrl elsewhere, preserving existing settings.
pub(crate) fn modifier_spec(mods: &Modifiers) -> Option<String> {
    if mods.function
        || (cfg!(target_os = "macos") && mods.control)
        || (!cfg!(target_os = "macos") && mods.platform)
    {
        return None;
    }
    let mut keys = Vec::new();
    if mods.secondary() {
        keys.push("ctrl");
    }
    if mods.alt {
        keys.push("alt");
    }
    if mods.shift {
        keys.push("shift");
    }
    Some(if keys.is_empty() {
        "none".into()
    } else {
        keys.join("+")
    })
}

impl WheelBindings {
    pub(crate) fn get(&self, action: WheelAction) -> &str {
        match action {
            WheelAction::PanVertical => &self.vertical,
            WheelAction::PanHorizontal => &self.horizontal,
            WheelAction::Zoom => &self.zoom,
        }
    }

    pub(crate) fn set(&mut self, action: WheelAction, value: String) {
        *match action {
            WheelAction::PanVertical => &mut self.vertical,
            WheelAction::PanHorizontal => &mut self.horizontal,
            WheelAction::Zoom => &mut self.zoom,
        } = value;
    }

    pub(crate) fn conflict(&self, target: WheelAction, value: &str) -> Option<WheelAction> {
        [
            WheelAction::PanVertical,
            WheelAction::PanHorizontal,
            WheelAction::Zoom,
        ]
        .into_iter()
        .find(|&other| other != target && self.get(other) == value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mods(ctrl: bool, alt: bool, shift: bool) -> Modifiers {
        Modifiers {
            control: ctrl && !cfg!(target_os = "macos"),
            platform: ctrl && cfg!(target_os = "macos"),
            alt,
            shift,
            ..Default::default()
        }
    }

    #[test]
    fn defaults_pan_scroll_and_zoom_on_alt() {
        let b = WheelBindings::default();
        assert_eq!(
            action(&mods(false, false, false), &b),
            Some(WheelAction::PanVertical)
        );
        assert_eq!(
            action(&mods(true, false, false), &b),
            Some(WheelAction::PanHorizontal)
        );
        assert_eq!(
            action(&mods(false, true, false), &b),
            Some(WheelAction::Zoom)
        );
        // Unbound combinations leave the canvas unchanged.
        assert_eq!(action(&mods(true, true, false), &b), None);
    }

    #[test]
    fn bindings_are_swappable() {
        let b = WheelBindings {
            vertical: "none".to_string(),
            horizontal: "shift".to_string(),
            zoom: "ctrl+alt".to_string(),
        };
        assert_eq!(
            action(&mods(false, false, true), &b),
            Some(WheelAction::PanHorizontal)
        );
        assert_eq!(
            action(&mods(true, true, false), &b),
            Some(WheelAction::Zoom)
        );
        assert_eq!(
            action(&mods(false, false, false), &b),
            Some(WheelAction::PanVertical)
        );
    }

    #[test]
    fn zoom_binding_takes_precedence() {
        let b = WheelBindings {
            vertical: "none".to_string(),
            horizontal: "alt".to_string(),
            zoom: "alt".to_string(),
        };
        assert_eq!(
            action(&mods(false, true, false), &b),
            Some(WheelAction::Zoom)
        );
    }

    #[test]
    fn vertical_binding_is_respected_and_unbound_wheel_is_ignored() {
        let b = WheelBindings {
            vertical: "ctrl+shift".into(),
            ..Default::default()
        };
        assert_eq!(
            action(&mods(true, false, true), &b),
            Some(WheelAction::PanVertical)
        );
        assert_eq!(action(&mods(false, false, false), &b), None);
    }

    #[test]
    fn recording_and_dispatch_share_exact_modifier_combinations() {
        for ctrl in [false, true] {
            for alt in [false, true] {
                for shift in [false, true] {
                    let keys = mods(ctrl, alt, shift);
                    let value = modifier_spec(&keys).unwrap();
                    let mut b = WheelBindings {
                        vertical: "disabled".into(),
                        horizontal: "disabled".into(),
                        zoom: "disabled".into(),
                    };
                    b.set(WheelAction::Zoom, value.clone());
                    assert_eq!(action(&keys, &b), Some(WheelAction::Zoom));
                    assert_eq!(b.conflict(WheelAction::Zoom, &value), None);
                    assert_eq!(
                        b.conflict(WheelAction::PanVertical, &value),
                        Some(WheelAction::Zoom)
                    );
                    let restored: WheelBindings =
                        serde_json::from_str(&serde_json::to_string(&b).unwrap()).unwrap();
                    assert_eq!(action(&keys, &restored), Some(WheelAction::Zoom));
                }
            }
        }
        let b = WheelBindings {
            zoom: "ctrl+alt".into(),
            ..Default::default()
        };
        assert_eq!(action(&mods(true, true, true), &b), None);
    }

    #[test]
    fn unsupported_modifiers_do_not_turn_into_plain_wheel() {
        let keys = Modifiers {
            function: true,
            ..Default::default()
        };
        assert_eq!(modifier_spec(&keys), None);
        assert_eq!(action(&keys, &WheelBindings::default()), None);
    }
}
