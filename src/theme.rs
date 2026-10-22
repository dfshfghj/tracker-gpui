use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use gpui_kit::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ThemeMode {
    Auto,
    Light,
    Dark,
}

pub(crate) static THEME_MODE: AtomicU8 = AtomicU8::new(0);
pub(crate) static LIGHT_THEME: AtomicBool = AtomicBool::new(false);

pub(crate) fn theme_mode() -> ThemeMode {
    match THEME_MODE.load(Ordering::Relaxed) {
        1 => ThemeMode::Light,
        2 => ThemeMode::Dark,
        _ => ThemeMode::Auto,
    }
}

pub(crate) fn light_theme() -> bool {
    LIGHT_THEME.load(Ordering::Relaxed)
}

pub(crate) fn set_theme_mode(mode: ThemeMode) {
    let light = match mode {
        ThemeMode::Auto => system_prefers_light(),
        ThemeMode::Light => true,
        ThemeMode::Dark => false,
    };
    LIGHT_THEME.store(light, Ordering::Relaxed);
    THEME_MODE.store(
        match mode {
            ThemeMode::Auto => 0,
            ThemeMode::Light => 1,
            ThemeMode::Dark => 2,
        },
        Ordering::Relaxed,
    );
}

/// Keep gpui-component widgets (notably TitleBar and its window controls)
/// in the same light/dark mode as the app's own palette.
pub(crate) fn sync_component_theme(cx: &mut App) {
    let mode = if light_theme() {
        gpui_component::ThemeMode::Light
    } else {
        gpui_component::ThemeMode::Dark
    };
    gpui_component::Theme::change(mode, None, cx);
}

#[cfg(target_os = "windows")]
pub(crate) fn system_prefers_light() -> bool {
    use std::ffi::c_void;

    const HKEY_CURRENT_USER: isize = 0x8000_0001u32 as i32 as isize;
    const RRF_RT_REG_DWORD: u32 = 0x10;

    #[link(name = "advapi32")]
    extern "system" {
        fn RegGetValueW(
            hkey: isize,
            sub_key: *const u16,
            value: *const u16,
            flags: u32,
            value_type: *mut u32,
            data: *mut c_void,
            data_size: *mut u32,
        ) -> i32;
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    // Spawning `reg.exe` from a GUI-subsystem process makes Windows create a
    // console for the child and blocks the UI thread for over a second.
    let sub_key = wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let value = wide("AppsUseLightTheme");
    let mut data = 0u32;
    let mut data_size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            sub_key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&mut data as *mut u32).cast(),
            &mut data_size,
        )
    };
    status == 0 && data == 1
}

#[cfg(target_os = "macos")]
pub(crate) fn system_prefers_light() -> bool {
    std::process::Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output()
        .ok()
        .map(|output| {
            !String::from_utf8_lossy(&output.stdout)
                .to_ascii_lowercase()
                .contains("dark")
        })
        .unwrap_or(true)
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub(crate) fn system_prefers_light() -> bool {
    false
}

/// Theme-aware replacement for `c(0x…)`. Unknown colors keep their value.
pub(crate) fn c(value: u32) -> Rgba {
    if !light_theme() {
        return rgb(value);
    }
    let mapped = match value {
        0x070b12 => 0xf2f5f9,
        0x0a1018 => 0xffffff,
        0x0d1420 => 0xf8fafc,
        0x111a29 => 0xe9eef5,
        0x152238 => 0xdce6f2,
        0x16233a => 0xd5e2f3,
        0x1b2b45 => 0xd2e4fb,
        0x1d2a3d => 0xd3dbe6,
        0x2a3b55 => 0xb7c4d7,
        0x0b1220 => 0xe7edf4,
        0x4a5a73 => 0xaab6c6,
        0xdbe6f5 => 0x1b2735,
        0xb9cbe4 => 0x28384c,
        0xc8d7f0 => 0x2a3b50,
        0xa9bad2 => 0x34465c,
        0x9fb2cc => 0x3d4f66,
        0x8fa4c0 => 0x465a71,
        0x8fa3bf => 0x4b5f76,
        0x7d8ca6 => 0x5a6c82,
        0x6f819c => 0x64758a,
        0x5f6f88 => 0x77879b,
        0x8ecbff => 0x1f6fd0,
        0x4dabf7 => 0x1f7ae0,
        0x06101c => 0xffffff,
        0xffd43b => 0xb07d00,
        0xffd166 => 0xb57d00,
        0xff6b6b => 0xd43a3a,
        0x51cf66 => 0x2f9e44,
        other => other,
    };
    rgb(mapped)
}
