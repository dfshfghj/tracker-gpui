use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=FFMPEG_DIR");

    // The bundled third_party FFmpeg is a Windows shared build. On macOS the
    // ffmpeg-sys crate discovers Homebrew FFmpeg via pkg-config instead.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let Ok(dir) = env::var("FFMPEG_DIR") else {
        return;
    };
    let bin = PathBuf::from(&dir).join("bin");
    if !bin.is_dir() {
        println!("cargo:warning=FFMPEG_DIR/bin not found: {}", bin.display());
        return;
    }

    let target = env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("target"));
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".into());
    let out = target.join(profile);
    if fs::create_dir_all(&out).is_err() {
        return;
    }

    let Ok(entries) = fs::read_dir(&bin) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let required = ["avcodec", "avformat", "avutil", "swscale", "swresample"];
        if name.ends_with(".dll") && required.iter().any(|lib| name.starts_with(lib)) {
            let _ = fs::copy(&path, out.join(name));
        }
    }

    // The official OpenCV Windows package ships one opencv_world DLL next to
    // the import libraries. Keep it beside the executable for cargo run.
    if let Ok(paths) = env::var("OPENCV_LINK_PATHS") {
        for dir in paths.split(';').filter(|dir| !dir.is_empty()) {
            let lib_dir = PathBuf::from(dir);
            let Some(prefix) = lib_dir.parent() else {
                continue;
            };
            let Ok(entries) = fs::read_dir(prefix.join("bin")) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name();
                let Some(name) = name.to_str() else { continue };
                if name.starts_with("opencv_world") && name.ends_with(".dll") && !name.ends_with("d.dll")
                {
                    let _ = fs::copy(entry.path(), out.join(name));
                }
            }
        }
    }
}
