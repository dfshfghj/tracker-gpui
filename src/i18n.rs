//! Locale selection, persistence and switching.
//!
//! Translations live in `locales/app.yml` (`_version: 2`) and are compiled into
//! the binary by `rust_i18n::i18n!` in `main.rs`. `rust_i18n` keeps the active
//! locale in a process-global, so switching it also switches the strings that
//! `gpui-component` renders (its `t!` calls read the same global).
//!
//! Nothing here repaints the UI: callers must call `cx.refresh_windows()`
//! after `set_locale`, otherwise the window keeps the old language until the
//! next unrelated redraw.

/// Translate `key` in the active locale.
///
/// Missing keys render as the key itself, so a half-migrated UI degrades to
/// identifiers instead of panicking. Use `t!("key", arg = value)` directly for
/// strings that need `%{arg}` interpolation.
pub(crate) fn tr(key: &str) -> String {
    rust_i18n::t!(key).to_string()
}

/// Wrap a lower-level (usually OpenCV) error into a user-visible message.
///
/// `op` is an English operation name such as `"morphology kernel"`; the format
/// string itself lives in the locale file so the sentence order can change per
/// language.
pub(crate) fn cv(op: &str, err: impl std::fmt::Display) -> String {
    rust_i18n::t!("msg.cv_failed", op = op, err = err).to_string()
}

/// Languages the UI is actually translated into.
pub(crate) const SUPPORTED: &[&str] = &["en", "zh-CN"];

/// Preference value meaning "follow the OS locale".
pub(crate) const AUTO: &str = "auto";

/// Endonym shown in the language menu; language names are never translated.
pub(crate) fn native_name(locale: &str) -> &'static str {
    match locale {
        "zh-CN" => "简体中文",
        "zh-TW" | "zh-HK" => "繁體中文",
        _ => "English",
    }
}

/// Map an arbitrary BCP-47 tag onto a locale we ship.
///
/// `zh-TW` / `zh-HK` fall back to Simplified Chinese for now: the alternative
/// would be English app strings next to Chinese component strings.
pub(crate) fn normalize(tag: &str) -> &'static str {
    let lower = tag.to_ascii_lowercase();
    if lower.starts_with("zh") {
        "zh-CN"
    } else {
        "en"
    }
}

/// The OS UI language, or `en` when it cannot be determined.
pub(crate) fn system_locale() -> &'static str {
    sys_locale::get_locale()
        .map(|tag| normalize(&tag))
        .unwrap_or("en")
}

/// Resolve a stored preference (`None` / `"auto"` = follow the system).
pub(crate) fn resolve(pref: Option<&str>) -> &'static str {
    match pref {
        Some(value) if !value.is_empty() && value != AUTO => normalize(value),
        _ => system_locale(),
    }
}

/// Activate `locale` for the whole process. Callers must repaint afterwards.
pub(crate) fn apply(locale: &str) {
    rust_i18n::set_locale(locale);
}

/// Rows of the View → Language menu: `(preference value, display label)`.
pub(crate) fn menu_entries() -> Vec<(&'static str, String)> {
    let mut entries = vec![(AUTO, tr("menu.item.language_auto"))];
    entries.extend(
        SUPPORTED
            .iter()
            .map(|locale| (*locale, native_name(locale).to_string())),
    );
    entries
}
