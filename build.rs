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
}
